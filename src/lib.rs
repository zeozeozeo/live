mod bot;
mod hooks;

use retour::static_detour;
use std::ffi::c_void;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{WindowFromDC, HDC},
    System::{
        Console::AllocConsole,
        LibraryLoader::{GetModuleHandleA, GetProcAddress},
        SystemServices::DLL_PROCESS_ATTACH,
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
#[no_mangle]
pub unsafe extern "system" fn DllMain(dll: u32, reason: u32, _reserved: *mut c_void) -> u32 {
    if reason == DLL_PROCESS_ATTACH {
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
    1
}

/// Main function
unsafe extern "system" fn zcblive_main(_dll: *mut c_void) -> u32 {
    // wait for enter key on panics
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info: &std::panic::PanicInfo<'_>| {
        panic_hook(info);
        let mut string = String::new();
        std::io::stdin().read_line(&mut string).unwrap();
        std::process::exit(1);
    }));

    AllocConsole().unwrap();
    simple_logger::SimpleLogger::new()
        .init()
        .expect("failed to initialize simple_logger");

    // get swapbuffers function
    let opengl = GetModuleHandleA(windows::core::s!("OPENGL32.dll")).unwrap();
    let swap_buffers: FnWglSwapBuffers =
        std::mem::transmute(GetProcAddress(opengl, windows::core::s!("wglSwapBuffers")));

    let (sx, rx) = std::sync::mpsc::channel();

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
                    egui::Window::new("www.sexmods.com/gd").show(ctx, |ui| {
                        for _ in 0..5 {
                            ui.horizontal(|ui| {
                                for _ in 0..5 {
                                    ui.label("Hello World!");
                                }
                            });
                        }
                    });
                }),
            )
            .expect("failed to call paint()");
            return h_wglSwapBuffers.call(hdc);
        })
        .unwrap()
        .enable()
        .unwrap();

    // wait until the closure sends hdc to us
    let hdc = rx.recv().unwrap();
    let hwnd = WindowFromDC(hdc);

    // set wndproc
    O_WNDPROC = Some(SetWindowLongPtrA(hwnd, GWLP_WNDPROC, h_wndproc as _));

    // start bot
    bot::BOT.run();

    0
}
