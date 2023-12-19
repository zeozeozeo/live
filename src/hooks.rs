use crate::BOT;
use geometrydash::{get_base, patch_mem, AddressUtils, GameManager, PlayLayer, PlayerObject, Ptr};
use retour::static_detour;
use std::ffi::c_void;

type FnPushButton = unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;
type FnReleaseButton = unsafe extern "fastcall" fn(PlayerObject, Ptr, i32) -> bool;
type FnPushButton2 = unsafe extern "fastcall" fn(PlayLayer, Ptr, i32, bool) -> u32;
type FnReleaseButton2 = unsafe extern "fastcall" fn(PlayLayer, Ptr, i32, bool) -> u32;
type FnInit = unsafe extern "fastcall" fn(PlayLayer, Ptr, Ptr) -> bool;
type FnQuit = unsafe extern "fastcall" fn(PlayLayer, Ptr);
type FnReset = unsafe extern "fastcall" fn(PlayLayer, Ptr);
type FnUpdate = unsafe extern "fastcall" fn(PlayLayer, Ptr, f32);
type FnOnEditor = unsafe extern "fastcall" fn(PlayLayer, Ptr, Ptr) -> *const c_void;

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

macro_rules! make_minhook_statics {
    ($($static:ident),+) => {
        $(
            #[allow(non_upper_case_globals)]
            static mut $static: *mut ::std::ffi::c_void = 0 as _;
        )*
    };
}

make_minhook_statics!(
    PushButton_MinHook,
    ReleaseButton_MinHook,
    PushButton2_MinHook,
    ReleaseButton2_MinHook,
    Init_MinHook,
    Quit_MinHook,
    Reset_MinHook,
    Update_MinHook,
    OnEditor_MinHook
);

/// Create a function wrapper without a specified calling convention
macro_rules! make_retour_fn {
    ($name:ident, $retour_name:ident($($($n:ident: $t:ty),+)?) $(-> $ret:ty)?) => {
        fn $retour_name($($($n: $t),+)?) $(-> $ret)? {
            unsafe { $name($($($n),+)?) }
        }
    };
}

macro_rules! call_hook {
    ($static:ident($($arg:expr),+), $typ:ty) => {
        if unsafe { BOT.used_minhook } {
            unsafe { ::std::mem::transmute::<*mut ::std::ffi::c_void, $typ>(concat_idents!($static, _MinHook))($($arg),+) }
        } else {
            unsafe { $static.call($($arg),+) }
        }
    };
}

unsafe extern "fastcall" fn push_button(player: PlayerObject, _edx: Ptr, button: i32) -> bool {
    let res = call_hook!(PushButton(player, 0, button), FnPushButton);
    unsafe { BOT.on_action(true, BOT.is_player2_obj(player)) };
    res
}

make_retour_fn!(push_button, push_button_retour(player: PlayerObject, _edx: Ptr, button: i32) -> bool);

unsafe extern "fastcall" fn release_button(player: PlayerObject, _edx: Ptr, button: i32) -> bool {
    let res = call_hook!(ReleaseButton(player, 0, button), FnReleaseButton);
    unsafe { BOT.on_action(false, BOT.is_player2_obj(player)) };
    res
}

make_retour_fn!(release_button, release_button_retour(player: PlayerObject, _edx: Ptr, button: i32) -> bool);

#[inline]
fn is_player1(playlayer: PlayLayer, button: bool) -> bool {
    let is2player = playlayer.level_settings().is_2player();
    let flip = is2player && GameManager::shared().get_game_variable("0010");
    !is2player || (button ^ flip)
}

unsafe extern "fastcall" fn push_button2(
    playlayer: PlayLayer,
    _edx: Ptr,
    param: i32,
    button: bool,
) -> u32 {
    let res = call_hook!(PushButton2(playlayer, 0, param, button), FnPushButton2);
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

make_retour_fn!(push_button2, push_button2_retour(playlayer: PlayLayer, _edx: Ptr, param: i32, button: bool) -> u32);

unsafe extern "fastcall" fn release_button2(
    playlayer: PlayLayer,
    _edx: Ptr,
    param: i32,
    button: bool,
) -> u32 {
    let res = call_hook!(
        ReleaseButton2(playlayer, 0, param, button),
        FnReleaseButton2
    );
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

make_retour_fn!(release_button2, release_button2_retour(playlayer: PlayLayer, _edx: Ptr, param: i32, button: bool) -> u32);

unsafe extern "fastcall" fn init(playlayer: PlayLayer, _edx: Ptr, level: Ptr) -> bool {
    let res = call_hook!(Init(playlayer, 0, level), FnInit);
    log::debug!("init");
    unsafe { BOT.playlayer = playlayer };
    unsafe { BOT.on_init() };
    res
}

make_retour_fn!(init, init_retour(playlayer: PlayLayer, _edx: Ptr, level: Ptr) -> bool);

unsafe extern "fastcall" fn quit(playlayer: PlayLayer, _edx: Ptr) {
    call_hook!(Quit(playlayer, 0), FnQuit);

    // set playlayer to null
    unsafe { BOT.playlayer = PlayLayer::from_address(0) };
}

make_retour_fn!(quit, quit_retour(playlayer: PlayLayer, _edx: Ptr));

unsafe extern "fastcall" fn reset(playlayer: PlayLayer, _edx: Ptr) {
    call_hook!(Reset(playlayer, 0), FnReset);

    if unsafe { BOT.playlayer.is_null() } && !playlayer.is_null() {
        log::debug!("reset init");
        unsafe { BOT.on_init() };
    }
    unsafe { BOT.playlayer = playlayer };

    log::debug!("reset");
    unsafe { BOT.on_reset() };
}

make_retour_fn!(reset, reset_retour(playlayer: PlayLayer, _edx: Ptr));

unsafe extern "fastcall" fn update(playlayer: PlayLayer, _edx: Ptr, dt: f32) {
    if unsafe { BOT.playlayer.is_null() } && !playlayer.is_null() {
        log::debug!("update init");
        unsafe { BOT.on_init() };
    }
    unsafe { BOT.playlayer = playlayer };

    call_hook!(Update(playlayer, 0, dt), FnUpdate);
}

make_retour_fn!(update, update_retour(playlayer: PlayLayer, _edx: Ptr, dt: f32));

unsafe extern "fastcall" fn on_editor(
    playlayer: PlayLayer,
    _edx: Ptr,
    param: Ptr,
) -> *const c_void {
    unsafe { BOT.playlayer = PlayLayer::from_address(0) };
    call_hook!(OnEditor(playlayer, 0, param), FnOnEditor)
}

make_retour_fn!(on_editor, on_editor_retour(playlayer: PlayLayer, _edx: Ptr, param: Ptr) -> *const c_void);

macro_rules! patch {
    ($addr:expr, $data:expr) => {
        let len = $data.len();
        let _ = patch_mem($addr, $data)
            .map_err(|e| log::error!("failed to write {len} bytes at {:#x}: {e}", $addr));
    };
}

pub fn anticheat_bypass() {
    log::info!("activating anticheat bypass");
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
    ($static:ident, $detour:ident, $addr:expr) => {
        let addr = ::geometrydash::get_base() + $addr;
        if unsafe { BOT.used_minhook } {
            ::log::info!("creating minhook hook: {} -> {:#x}", stringify!($static), $addr);
            concat_idents!($static, _MinHook) = ::std::mem::transmute(
                ::minhook::MinHook::create_hook(addr as _, $detour as _)
                    .expect(stringify!(failed to hook $static with minhook))
            );
        } else {
            ::log::info!("initializing retour hook: {} -> {:#x}", stringify!($static), $addr);
            $static
                .initialize(::std::mem::transmute(addr), concat_idents!($detour, _retour))
                .expect(stringify!(failed to hook $static with retour));
            ::log::info!("enabling retour hook: {} -> {:#x}", stringify!($static), $addr);
            $static
                .enable()
                .expect(stringify!(failed to enable $static retour hook));
        }
    };
}

pub unsafe fn init_hooks() {
    if unsafe { BOT.conf.hook_wait } {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    anticheat_bypass();

    let alternate = unsafe { BOT.conf.use_alternate_hook };

    if !alternate {
        hook!(PushButton, push_button, 0x1F4E40);
        hook!(ReleaseButton, release_button, 0x1F4F70);
    } else {
        hook!(PushButton2, push_button2, 0x111500);
        hook!(ReleaseButton2, release_button2, 0x111660);
    }

    hook!(Init, init, 0x1fb780);
    hook!(Quit, quit, 0x20D810);
    hook!(Reset, reset, 0x20BF00);
    hook!(Update, update, 0x2029C0);
    hook!(OnEditor, on_editor, 0x1E60E0);

    if unsafe { BOT.used_minhook } {
        log::info!("enabling all minhook hooks");
        unsafe { minhook::MinHook::enable_all_hooks().expect("failed to enable hooks") };
    }
}

macro_rules! disable_hooks {
    ($($static:ident),+) => {
        $(
            if unsafe { BOT.used_minhook } {
                log::info!("disabling {} minhook hook", stringify!($static));
                let _ = ::minhook::MinHook::disable_hook(::std::mem::transmute(concat_idents!($static, _MinHook)))
                    .map_err(|e| log::error!("failed to disable {} minhook hook: {e:?}", stringify!($static)));
            } else {
                log::info!("disabling {} retour hook", stringify!($static));
                let _ = unsafe { $static.disable() }
                    .map_err(|e| log::error!("failed to disable {} hook: {e}", stringify!($static)));
            }
        )*
    };
}

pub unsafe fn disable_hooks() {
    log::info!("disabling hooks");

    if unsafe { BOT.used_alternate_hook } {
        disable_hooks!(PushButton2, ReleaseButton2);
    } else {
        disable_hooks!(PushButton, ReleaseButton);
    }

    disable_hooks!(Init, Quit, Reset, Update, OnEditor);

    if unsafe { BOT.used_minhook } {
        log::info!("uninitializing minhook");
        minhook::MinHook::uninitialize();
    }
}
