#![feature(concat_idents)]

mod bot;
mod hooks;
mod utils;
mod game_manager;

use bot::BOT;
use egui_opengl_internal::OpenGLApp;
use retour::static_detour;
use std::{ffi::c_void, sync::Once};
use windows::Win32::{
    Foundation::{BOOL, HMODULE, HWND, LPARAM, LRESULT, TRUE, WPARAM},
    Graphics::Gdi::{WindowFromDC, HDC},
    System::{
        LibraryLoader::FreeLibraryAndExitThread,
        SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
        Threading::{CreateThread, THREAD_CREATION_FLAGS},
    },
    UI::WindowsAndMessaging::{CallWindowProcA, SetWindowLongPtrA, GWLP_WNDPROC},
};

// wglSwapBuffers detour
static_detour! {
    static h_wglSwapBuffers: unsafe extern "system" fn(HDC) -> i32;
}

/// wglSwapBuffers function type
type FnWglSwapBuffers = unsafe extern "system" fn(HDC) -> i32;

/// returned from SetWindowLongPtrA
static mut O_WNDPROC: Option<i32> = None;
static mut EGUI_APP: OpenGLApp<i32> = OpenGLApp::new();

unsafe fn h_wndproc_old(hwnd: HWND, umsg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        log::info!("CallWindowProcW hooked (old)");
    });

    let egui_wants_input = EGUI_APP.wnd_proc(umsg, wparam, lparam);
    if egui_wants_input {
        return LRESULT(1);
    }

    CallWindowProcA(
        std::mem::transmute(O_WNDPROC.unwrap()),
        hwnd,
        umsg,
        wparam,
        lparam,
    )
}

/// WNDPROC hook
#[no_mangle]
unsafe extern "system" fn h_wndproc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        log::info!("CallWindowProcW hooked (new)");
    });

    if egui_gl_hook::is_init() {
        let should_skip_wnd_proc = egui_gl_hook::on_event(umsg, wparam.0, lparam.0).unwrap();

        if should_skip_wnd_proc {
            return LRESULT(1);
        }
    }

    CallWindowProcA(
        std::mem::transmute(O_WNDPROC.unwrap()),
        hwnd,
        umsg,
        wparam,
        lparam,
    )
}

/// DLL entrypoint
///
/// # Safety
#[no_mangle]
pub unsafe extern "system" fn DllMain(dll: u32, reason: u32, _reserved: *mut c_void) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            CreateThread(
                None,
                0,
                Some(zcblive_main),
                Some(dll as _),
                THREAD_CREATION_FLAGS(0),
                None,
            )
            .unwrap();
        }
        DLL_PROCESS_DETACH => {
            hooks::disable_hooks();
            FreeLibraryAndExitThread(std::mem::transmute::<_, HMODULE>(dll), 0);
        }
        _ => {}
    }
    TRUE
}

fn hk_wgl_swap_buffers_old(hdc: HDC) -> i32 {
    unsafe {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            log::info!("wglSwapBuffers hooked (old)");
            let window = WindowFromDC(hdc);
            EGUI_APP.init_default(hdc, window, |ctx, _| BOT.draw_ui(ctx));

            O_WNDPROC = Some(std::mem::transmute(SetWindowLongPtrA(
                window,
                GWLP_WNDPROC,
                h_wndproc_old as usize as i32,
            )));
        });

        EGUI_APP.render(hdc);
        h_wglSwapBuffers.call(hdc)
    }
}

fn hk_wgl_swap_buffers(hdc: HDC) -> i32 {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        log::info!("wglSwapBuffers hooked (new)");
    });

    unsafe {
        if hdc == HDC(0) {
            return h_wglSwapBuffers.call(hdc);
        }

        // initialize egui_gl_hook
        if !egui_gl_hook::is_init() {
            let hwnd = WindowFromDC(hdc);
            O_WNDPROC = Some(SetWindowLongPtrA(
                hwnd,
                GWLP_WNDPROC,
                h_wndproc as usize as i32,
            ));
            egui_gl_hook::init(hdc).unwrap();
        }

        // paint this frame
        egui_gl_hook::paint(
            hdc,
            Box::new(|ctx| {
                BOT.draw_ui(ctx);
            }),
        )
        .expect("failed to call paint()");
        h_wglSwapBuffers.call(hdc)
    }
}

/// Main function
#[no_mangle]
unsafe extern "system" fn zcblive_main(_hmod: *mut c_void) -> u32 {
    // wait for enter key on panics
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info: &std::panic::PanicInfo<'_>| {
        panic_hook(info);
        let mut string = String::new();
        std::io::stdin().read_line(&mut string).unwrap();
        std::process::exit(1);
    }));

    BOT.maybe_alloc_console();

    // get swapbuffers function
    let swap_buffers: FnWglSwapBuffers = std::mem::transmute(
        egui_opengl_internal::utils::get_proc_address("wglSwapBuffers"),
    );

    log::info!("wglSwapBuffers: {:#X}", swap_buffers as usize);

    // initialize swapbuffers hook
    h_wglSwapBuffers
        .initialize(
            swap_buffers,
            if BOT.used_old_egui_hook {
                hk_wgl_swap_buffers_old
            } else {
                hk_wgl_swap_buffers
            },
        )
        .unwrap()
        .enable()
        .unwrap();

    // init bot
    BOT.init();
    1
}

// functions for other mods to call if they need it for some reason

#[no_mangle]
#[inline(never)]
unsafe extern "system" fn zcblive_action_callback(push: bool, player2: bool) {
    BOT.on_action(push, player2)
}

#[no_mangle]
#[inline(never)]
unsafe extern "system" fn zcblive_set_playlayer(playlayer: *mut c_void /*PlayLayer*/) {
    BOT.playlayer = playlayer;
}
