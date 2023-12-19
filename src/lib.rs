#![feature(concat_idents)]

mod bot;
mod hooks;
mod utils;

use bot::BOT;
use retour::static_detour;
use std::ffi::c_void;
use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM, LRESULT, TRUE, WPARAM},
    Graphics::Gdi::{WindowFromDC, HDC},
    System::{
        LibraryLoader::{GetModuleHandleA, GetProcAddress},
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

/// WNDPROC hook
#[no_mangle]
unsafe extern "system" fn h_wndproc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
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
        }
        _ => {}
    }
    TRUE
}

/// Main function
#[no_mangle]
unsafe extern "system" fn zcblive_main(_dll: *mut c_void) -> u32 {
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
    let opengl = GetModuleHandleA(windows::core::s!("OPENGL32.dll")).unwrap();
    let swap_buffers: FnWglSwapBuffers =
        std::mem::transmute(GetProcAddress(opengl, windows::core::s!("wglSwapBuffers")));

    let (sx, rx) = std::sync::mpsc::channel();

    // init bot
    BOT.init();

    // initialize swapbuffers hook
    h_wglSwapBuffers
        .initialize(swap_buffers, move |hdc| {
            if hdc == HDC(0) {
                return h_wglSwapBuffers.call(hdc);
            }

            // initialize egui_gl_hook
            if !egui_gl_hook::is_init() {
                sx.send(hdc).unwrap();
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
        })
        .unwrap()
        .enable()
        .unwrap();

    // wait until the closure sends hdc to us
    let hdc = rx.recv().unwrap();
    let hwnd = WindowFromDC(hdc);

    // set wndproc
    O_WNDPROC = Some(SetWindowLongPtrA(
        hwnd,
        GWLP_WNDPROC,
        h_wndproc as usize as i32,
    ));
    0
}

// functions for other mods to call if they need it for some reason

#[no_mangle]
#[inline(never)]
unsafe extern "system" fn zcblive_action_callback(push: bool, player2: bool) {
    BOT.on_action(push, player2)
}

#[no_mangle]
#[inline(never)]
unsafe extern "system" fn zcblive_set_playlayer(playlayer: geometrydash::PlayLayer) {
    BOT.playlayer = playlayer;
}
