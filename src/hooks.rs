use crate::BOT;
use geometrydash::{get_base, patch_mem, AddressUtils, GameManager, PlayLayer, PlayerObject, Ptr};
use retour::static_detour;
use std::ffi::c_void;

static_detour! {
    static PushButton: unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;
    static ReleaseButton: unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;
    static PushButton2: unsafe extern "fastcall" fn(PlayLayer, Ptr, i32, bool) -> u32;
    static ReleaseButton2: unsafe extern "fastcall" fn(PlayLayer, Ptr, i32, bool) -> u32;
    static Init: unsafe extern "fastcall" fn(PlayLayer, Ptr, Ptr) -> bool;
    static Quit: unsafe extern "fastcall" fn(PlayLayer, Ptr);
    static Reset: unsafe extern "fastcall" fn(PlayLayer, Ptr);
    static Update: unsafe extern "fastcall" fn(PlayLayer, Ptr, f32);
    static OnEditor: unsafe extern "fastcall" fn(PlayLayer, Ptr, Ptr) -> *const c_void;
}

fn push_button(player: PlayerObject, _edx: Ptr, button: i32) -> bool {
    let res = unsafe { PushButton.call(player, 0, button) };
    // log::info!("pushbutton: {button}, ");
    unsafe { BOT.on_action(true, BOT.is_player2_obj(player)) };
    res
}

fn release_button(player: PlayerObject, _edx: Ptr, button: i32) -> bool {
    let res = unsafe { ReleaseButton.call(player, 0, button) };
    // log::info!("releasebutton: {button}");
    unsafe { BOT.on_action(false, BOT.is_player2_obj(player)) };
    res
}

#[inline]
fn is_player1(playlayer: PlayLayer, button: bool) -> bool {
    let is2player = playlayer.level_settings().is_2player();
    let flip = is2player && GameManager::shared().get_game_variable("0010");
    !is2player || (button ^ flip)
}

fn push_button2(playlayer: PlayLayer, _edx: Ptr, param: i32, button: bool) -> u32 {
    let res = unsafe { PushButton2.call(playlayer, 0, param, button) };
    if unsafe { BOT.playlayer.is_null() } && !playlayer.is_null() {
        log::debug!("push2 init");
        unsafe { BOT.on_init() };
    }
    unsafe { BOT.playlayer = playlayer };

    if unsafe { BOT.conf.use_alternate_hook } {
        unsafe { BOT.on_action(true, !is_player1(playlayer, button)) };
    }
    res
}

fn release_button2(playlayer: PlayLayer, _edx: Ptr, param: i32, button: bool) -> u32 {
    let res = unsafe { ReleaseButton2.call(playlayer, 0, param, button) };
    if unsafe { BOT.playlayer.is_null() } && !playlayer.is_null() {
        log::debug!("release2 init");
        unsafe { BOT.on_init() };
    }
    unsafe { BOT.playlayer = playlayer };

    if unsafe { BOT.conf.use_alternate_hook } {
        unsafe { BOT.on_action(false, !is_player1(playlayer, button)) };
    }
    res
}

fn init(playlayer: PlayLayer, _edx: Ptr, level: Ptr) -> bool {
    let res = unsafe { Init.call(playlayer, 0, level) };
    log::debug!("init");
    unsafe { BOT.playlayer = playlayer };
    unsafe { BOT.on_init() };
    res
}

fn quit(playlayer: PlayLayer, _edx: Ptr) {
    unsafe { Quit.call(playlayer, 0) };

    // set playlayer to null
    unsafe { BOT.playlayer = PlayLayer::from_address(0) };
}

fn reset(playlayer: PlayLayer, _edx: Ptr) {
    unsafe { Reset.call(playlayer, 0) };

    if unsafe { BOT.playlayer.is_null() } && !playlayer.is_null() {
        log::debug!("reset init");
        unsafe { BOT.on_init() };
    }
    unsafe { BOT.playlayer = playlayer };

    log::debug!("reset");
    unsafe { BOT.on_reset() };
}

fn update(playlayer: PlayLayer, _edx: Ptr, dt: f32) {
    if unsafe { BOT.playlayer.is_null() } && !playlayer.is_null() {
        log::debug!("update init");
        unsafe { BOT.on_init() };
    }
    unsafe { BOT.playlayer = playlayer };
    unsafe { BOT.on_update() };
    unsafe { Update.call(playlayer, 0, dt) };
}

fn on_editor(playlayer: PlayLayer, _edx: Ptr, param: Ptr) -> *const c_void {
    unsafe { BOT.playlayer = PlayLayer::from_address(0) };
    unsafe { OnEditor.call(playlayer, 0, param) }
}

macro_rules! patch {
    ($addr:expr, $data:expr) => {
        let len = $data.len();
        let _ = patch_mem($addr, $data)
            .map_err(|e| log::error!("failed to write {len} bytes at {:#x}: {e}", $addr));
    };
}

pub fn anticheat_bypass() {
    log::trace!("activating anticheat bypass");
    patch!(get_base() + 0x202aaa, &[0xeb, 0x2e]);
    patch!(get_base() + 0x15fc2e, &[0xeb]);
    patch!(get_base() + 0x1fd557, &[0xeb, 0x0c]);
    patch!(
        get_base() + 0x1fd742,
        &[
            0xc7, 0x87, 0xe0, 0x02, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xc7, 0x87, 0xe4, 0x02,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
        ]
    );
    patch!(get_base() + 0x1fd756, &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90]);
    patch!(get_base() + 0x1fd79a, &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90]);
    patch!(get_base() + 0x1fd7af, &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90]);
    patch!(get_base() + 0x20d3b3, &[0x90, 0x90, 0x90, 0x90, 0x90]);
    patch!(get_base() + 0x1ff7a2, &[0x90, 0x90]);
    patch!(get_base() + 0x18b2b4, &[0xb0, 0x01]);
    patch!(get_base() + 0x20c4e6, &[0xe9, 0xd7, 0x00, 0x00, 0x00, 0x90]);
}

macro_rules! hook {
    ($static:expr, $detour:expr, $addr:expr) => {
        let addr = ::geometrydash::get_base() + $addr;
        if unsafe { BOT.conf.use_minhook } {
            ::minhook::MinHook::create_hook(addr as _, $detour as _)
                .expect(stringify!(failed to hook $static));
        } else {
            $static
                .initialize(::std::mem::transmute(addr), $detour)
                .expect(stringify!(failed to hook $static));
            $static
                .enable()
                .expect(stringify!(failed to enable $static hook));
        }
    };
}

pub unsafe fn init_hooks() {
    if unsafe { BOT.conf.hook_wait } {
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    anticheat_bypass();

    let alternate = unsafe { BOT.conf.use_alternate_hook };
    let use_retour = !unsafe { BOT.conf.use_minhook };

    if !alternate {
        hook!(PushButton, push_button, 0x1F4E40);
        hook!(ReleaseButton, release_button, 0x1F4F70);
    } else {
        hook!(PushButton2, push_button2, 0x111500);
        hook!(PushButton2, release_button2, 0x111660);
    }

    hook!(Init, init, 0x1fb780);
    hook!(Quit, quit, 0x20D810);
    hook!(Reset, reset, 0x20BF00);
    hook!(Update, update, 0x2029C0);
    hook!(OnEditor, on_editor, 0x1E60E0);

    if !use_retour {
        unsafe { minhook::MinHook::enable_all_hooks().expect("failed to enable hooks") };
    }
}

pub unsafe fn disable_hooks() {
    log::debug!("disabling hooks");
    let alternate = unsafe { BOT.used_alternate_hook };

    if !alternate {
        let _ = unsafe { PushButton.disable() }
            .map_err(|e| log::error!("failed to disable PushButton hook: {e}"));
        let _ = unsafe { ReleaseButton.disable() }
            .map_err(|e| log::error!("failed to disable ReleaseButton hook: {e}"));
    } else {
        let _ = unsafe { PushButton2.disable() }
            .map_err(|e| log::error!("failed to disable PushButton2 hook: {e}"));
        let _ = unsafe { ReleaseButton2.disable() }
            .map_err(|e| log::error!("failed to disable ReleaseButton2 hook: {e}"));
    }

    let _ = unsafe { Init.disable() }.map_err(|e| log::error!("failed to disable Init hook: {e}"));
    let _ = unsafe { Quit.disable() }.map_err(|e| log::error!("failed to disable Quit hook: {e}"));
    let _ =
        unsafe { Reset.disable() }.map_err(|e| log::error!("failed to disable Reset hook: {e}"));
    // let _ = unsafe { InitFMOD.disable() }
    //     .map_err(|e| log::error!("failed to disable InitFMOD hook: {e}"));
    let _ =
        unsafe { Update.disable() }.map_err(|e| log::error!("failed to disable Update hook: {e}"));
    let _ = unsafe { OnEditor.disable() }
        .map_err(|e| log::error!("failed to disable OnEditor hook: {e}"));

    // minhook::MinHook::uninitialize();
}
