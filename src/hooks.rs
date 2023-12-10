use crate::BOT;
use geometrydash::{get_base, patch_mem, AddressUtils, GameManager, PlayLayer, PlayerObject, Ptr};
use retour::static_detour;

// pushButton/releaseButton methods that take [PlayerObject].

type FnPushButton = unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;
type FnReleaseButton = unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;

type FnPushButton2 = unsafe extern "fastcall" fn(PlayLayer, Ptr, i32, bool) -> u32;
type FnReleaseButton2 = unsafe extern "fastcall" fn(PlayLayer, Ptr, i32, bool) -> u32;

/// called when entering a level.
type FnInit = unsafe extern "fastcall" fn(PlayLayer, Ptr, Ptr) -> bool;

/// called when exiting from a level.
type FnQuit = unsafe extern "fastcall" fn(PlayLayer, Ptr);

type FnReset = unsafe extern "fastcall" fn(PlayLayer, Ptr);

static_detour! {
    static PushButton: unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;
    static ReleaseButton: unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;
    static PushButton2: unsafe extern "fastcall" fn(PlayLayer, Ptr, i32, bool) -> u32;
    static ReleaseButton2: unsafe extern "fastcall" fn(PlayLayer, Ptr, i32, bool) -> u32;
    static Init: unsafe extern "fastcall" fn(PlayLayer, Ptr, Ptr) -> bool;
    static Quit: unsafe extern "fastcall" fn(PlayLayer, Ptr);
    static Reset: unsafe extern "fastcall" fn(PlayLayer, Ptr);

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
    if !playlayer.is_null() {
        unsafe { BOT.playlayer = playlayer };
    }
    unsafe { BOT.on_action(true, !is_player1(playlayer, button)) };
    res
}

fn release_button2(playlayer: PlayLayer, _edx: Ptr, param: i32, button: bool) -> u32 {
    let res = unsafe { ReleaseButton2.call(playlayer, 0, param, button) };
    if !playlayer.is_null() {
        unsafe { BOT.playlayer = playlayer };
    }
    unsafe { BOT.on_action(false, !is_player1(playlayer, button)) };
    res
}

fn init(playlayer: PlayLayer, _edx: Ptr, level: Ptr) -> bool {
    let res = unsafe { Init.call(playlayer, 0, level) };

    // update playlayer, call oninit
    if res {
        unsafe { BOT.playlayer = playlayer };
        unsafe { BOT.oninit() };
    }

    res
}

fn quit(playlayer: PlayLayer, _edx: Ptr) {
    unsafe { Quit.call(playlayer, 0) };

    // set playlayer to null
    unsafe { BOT.playlayer = PlayLayer::from_address(0) };
}

fn reset(playlayer: PlayLayer, _edx: Ptr) {
    unsafe { Reset.call(playlayer, 0) };
    unsafe { BOT.onreset() };
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

pub unsafe fn init_hooks() {
    use std::mem::transmute;
    anticheat_bypass();

    let alternate = unsafe { BOT.conf.use_alternate_hook };

    if !alternate {
        // pushbutton
        let push_button_fn: FnPushButton = transmute(get_base() + 0x1F4E40);
        PushButton
            .initialize(push_button_fn, push_button)
            .expect("failed to hook PushButton");
        PushButton
            .enable()
            .expect("failed to enable PushButton hook");

        // releasebutton (same type as FnPushButton)
        let release_button_fn: FnReleaseButton = transmute(get_base() + 0x1F4F70);
        ReleaseButton
            .initialize(release_button_fn, release_button)
            .expect("failed to hook ReleaseButton");
        ReleaseButton
            .enable()
            .expect("failed to enable ReleaseButton hook");
    } else {
        // pushbutton2
        let push_button_fn2: FnPushButton2 = transmute(get_base() + 0x111500);
        PushButton2
            .initialize(push_button_fn2, push_button2)
            .expect("failed to hook PushButton2");
        PushButton2
            .enable()
            .expect("failed to enable PushButton2 hook");

        // releasebutton2 (same type as FnPushButton2)
        let release_button_fn2: FnReleaseButton2 = transmute(get_base() + 0x111660);
        ReleaseButton2
            .initialize(release_button_fn2, release_button2)
            .expect("failed to hook ReleaseButton2");
        ReleaseButton2
            .enable()
            .expect("failed to enable ReleaseButton2 hook");
    }

    // init
    let init_fn: FnInit = transmute(get_base() + 0x01FB780);
    Init.initialize(init_fn, init).expect("failed to hook Init");
    Init.enable().expect("failed to enable Init hook");

    // quit
    let quit_fn: FnQuit = transmute(get_base() + 0x20D810);
    Quit.initialize(quit_fn, quit).expect("failed to hook Quit");
    Quit.enable().expect("failed to enable Quit hook");

    // reset
    let reset_fn: FnReset = transmute(get_base() + 0x20BF00);
    Reset
        .initialize(reset_fn, reset)
        .expect("failed to hook Reset");
    Reset.enable().expect("failed to enable Reset hook");
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
}
