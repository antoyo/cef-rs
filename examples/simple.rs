#![allow(non_camel_case_types)]

extern crate cef;
extern crate gdk;
extern crate gdk_sys;
extern crate glib;
extern crate gtk;

use std::alloc::{Layout, dealloc};
use std::env;
use std::ffi::{CString, c_void};
use std::mem;
use std::process;
use std::ptr;

use gdk::Screen;
use gdk_sys::{
    GdkDisplay,
    GdkScreen,
    GdkVisual,
    GdkWindow,
};
use glib::translate::ToGlibPtr;
use gtk::{
    ContainerExt,
    Inhibit,
    Orientation,
    WidgetExt,
    Window,
    WindowType,
};

fn main() {
    let args = env::args();
    let args: Vec<_> = args.map(|string| CString::new(string).expect("c string"))
        .collect();
    let mut args: Vec<_> = args.iter()
        .map(|c_string| c_string.as_bytes() as *const _ as *mut _)
        .collect();
    let argc = args.len() as i32;
    let argv = args.as_mut_ptr();

    let main_args = cef_main_args_t {
        argc,
        argv,
    };

    let mut app: cef_app_t = unsafe { mem::zeroed() };
    app.base.size = mem::size_of::<cef_app_t>();
    app.on_before_command_line_processing = on_before_command_line_processing;
    app.on_register_custom_schemes = on_register_custom_schemes;
    app.get_resource_bundle_handler = get_resource_bundle_handler;
    app.get_browser_process_handler = get_browser_process_handler;
    app.get_render_process_handler = get_render_process_handler;

    let exit_code = unsafe { cef_execute_process(&main_args, &mut app, ptr::null_mut()) };
    if exit_code >= 0 {
        process::exit(exit_code);
    }

    /*XSetErrorHandler(XErrorHandlerImpl);
    XSetIOErrorHandler(XIOErrorHandlerImpl);*/

    let mut settings: cef_settings_t = unsafe { mem::zeroed() };
    settings.size = mem::size_of::<cef_settings_t>();
    unsafe { cef_initialize(&main_args, &settings, &mut app, ptr::null_mut()) };


    gtk::init().expect("gtk init");

    let window = Window::new(WindowType::Toplevel);

    let vbox = gtk::Box::new(Orientation::Vertical, 0);
    window.add(&vbox);
    fix_default_x11_visual(&window);
    window.show_all();

    window.connect_delete_event(|_, _| {
        unsafe { cef_quit_message_loop() };
        //gtk::main_quit();
        Inhibit(false)
    });

    let window = vbox.get_window();
    let xid = unsafe { gdk_x11_window_get_xid(window.to_glib_none().0) };

    let mut window_info: cef_window_info_t = unsafe { mem::zeroed() };
    window_info.parent_window = xid;

    unsafe { cef_run_message_loop() };
    unsafe { cef_shutdown() };

    // FIXME: not sure how the gtk main loop runs.
    gtk::main();
}

extern "C" {
    fn gdk_x11_display_get_xdisplay(display: *mut GdkDisplay) -> *mut Display;
    fn gdk_x11_screen_get_screen_number(screen: *mut GdkScreen) -> i32;
    fn gdk_x11_visual_get_xvisual(visual: *mut GdkVisual) -> *mut Visual;
    fn gdk_x11_window_get_xid(window: *mut GdkWindow) -> u64;
}

#[link(name="X11")]
extern "C" {
    fn XDefaultVisual(_2: *mut Display, _1: i32) -> *mut Visual;
}

enum Display {
}

enum XExtData {
}

#[repr(C)]
pub struct Visual {
    ext_data: *mut XExtData,
    visualid: u64,
    class: i32,
    red_mask: u64,
    green_mask: u64,
    blue_mask: u64,
    bits_per_rgb: i32,
    map_entries: i32,
}

fn fix_default_x11_visual(window: &Window) {
    if let Some(screen) = Screen::get_default() {
        unsafe {
            let visuals = screen.list_visuals();
            let display = screen.get_display();
            let xdisplay = gdk_x11_display_get_xdisplay(display.to_glib_none().0);
            let screen_number = gdk_x11_screen_get_screen_number(screen.to_glib_none().0);
            let default_xvisual = XDefaultVisual(xdisplay, screen_number);

            for visual in &visuals {
                if (*default_xvisual).visualid == (*gdk_x11_visual_get_xvisual(visual.to_glib_none().0)).visualid {
                    window.set_visual(visual);
                    break;
                }
            }
        }
    }
}

// Cef C API.

#[repr(C)]
struct cef_main_args_t {
    argc: i32,
    argv: *mut *mut i8,
}

#[repr(C)]
struct cef_settings_t {
    size: usize,
    no_sandbox: i32,
    browser_subprocess_path: cef_string_t,
    framework_dir_path: cef_string_t,
    multi_threaded_message_loop: i32,
    external_message_pump: i32,
    windowless_rendering_enabled: i32,
    command_line_args_disabled: i32,
    cache_path: cef_string_t,
    user_data_path: cef_string_t,
    persist_session_cookies: i32,
    persist_user_preferences: i32,
    user_agent: cef_string_t ,
    product_version: cef_string_t,
    locale: cef_string_t,
    log_file: cef_string_t,
    log_severity: cef_log_severity_t,
    javascript_flags: cef_string_t,
    resources_dir_path: cef_string_t,
    locales_dir_path: cef_string_t,
    pack_loading_disabled: i32,
    remote_debugging_port: i32,
    uncaught_exception_stack_size: i32,
    ignore_certificate_errors: i32,
    enable_net_security_expiration: i32,
    background_color: cef_color_t,
    accept_language_list: cef_string_t,
}

#[repr(C)]
struct cef_string_t {
    str: *const i8,
    length: usize,
    dtor: extern fn(str: *const i8),
}

trait ToCefString: ToString {
    fn to_cef_string(&self) -> cef_string_t {
        let string = self.to_string();
        let str = string.as_ptr() as *const i8;
        let length = string.len();
        mem::forget(string);

        extern fn dtor(str: *const i8) {
            if let Ok(layout) = Layout::from_size_align(mem::size_of::<i8>(), mem::align_of::<i8>()) {
                // TODO: make sure this is okay.
                unsafe {
                    dealloc(str as *mut u8, layout);
                }
            }
        }

        cef_string_t {
            str,
            length,
            dtor,
        }
    }
}

impl<'a> ToCefString for &'a str {
}

type cef_color_t = u32;

#[repr(C)]
enum cef_log_severity_t {
  LOGSEVERITY_DEFAULT,
  LOGSEVERITY_VERBOSE,
  //LOGSEVERITY_DEBUG = cef_log_severity_t::LOGSEVERITY_VERBOSE as isize,
  LOGSEVERITY_INFO,
  LOGSEVERITY_WARNING,
  LOGSEVERITY_ERROR,
  LOGSEVERITY_DISABLE = 99
}

#[repr(C)]
struct cef_app_t {
    base: cef_base_ref_counted_t,
    on_before_command_line_processing: extern fn(_self: *mut cef_app_t, process_type: *const cef_string_t, command_line: *mut cef_command_line_t),
    on_register_custom_schemes: extern fn(_self: *mut cef_app_t, registrar: *mut cef_scheme_registrar_t),
    get_resource_bundle_handler: extern fn(_self: *mut cef_app_t) -> *mut cef_resource_bundle_handler_t,
    get_browser_process_handler: extern fn(_self: *mut cef_app_t) -> *mut cef_browser_process_handler_t,
    get_render_process_handler: extern fn(_self: *mut cef_app_t) -> *mut cef_render_process_handler_t,
}

#[repr(C)]
struct cef_base_ref_counted_t {
    size: usize,
    add_ref: extern fn (_self: *const cef_base_ref_counted_t),
    release: extern fn(_self: *const cef_base_ref_counted_t) -> i32,
    has_one_ref: extern fn(_self: *const cef_base_ref_counted_t) -> i32,
    has_at_least_one_ref: extern fn(_self: *const cef_base_ref_counted_t) -> i32,
}

#[link(name ="cef")]
extern "C" {
    fn cef_execute_process(args: *const cef_main_args_t, application: *mut cef_app_t, windows_sandbox_info: *mut c_void)
        -> i32;
    fn cef_initialize(args: *const cef_main_args_t, settings: *const cef_settings_t, application: *mut cef_app_t,
        windows_sandbox_info: *mut c_void) -> i32;
    fn cef_run_message_loop();
    fn cef_quit_message_loop();
    fn cef_shutdown();
    fn cef_browser_view_create(client: *mut cef_client_t, url: *const cef_string_t,
        settings: *const cef_browser_settings_t, request_context: *mut cef_request_context_t,
        delegate: *mut cef_browser_view_delegate_t) -> *mut cef_browser_view_t;
    fn cef_window_create_top_level(delegate: *mut cef_window_delegate_t) -> *mut cef_window_t;
}

#[repr(C)]
enum cef_transition_type_t {
  TT_LINK = 0,
  TT_EXPLICIT = 1,
  TT_AUTO_SUBFRAME = 3,
  TT_MANUAL_SUBFRAME = 4,
  TT_FORM_SUBMIT = 7,
  TT_RELOAD = 8,
  TT_SOURCE_MASK = 0xFF,
  TT_BLOCKED_FLAG = 0x00800000,
  TT_FORWARD_BACK_FLAG = 0x01000000,
  TT_CHAIN_START_FLAG = 0x10000000,
  TT_CHAIN_END_FLAG = 0x20000000,
  TT_CLIENT_REDIRECT_FLAG = 0x40000000,
  TT_SERVER_REDIRECT_FLAG = 0x80000000,
  TT_IS_REDIRECT_MASK = 0xC0000000,
  TT_QUALIFIER_MASK = 0xFFFFFF00,
}

#[repr(C)]
enum cef_errorcode_t {
  ERR_NONE = 0,
  ERR_FAILED = -2,
  ERR_ABORTED = -3,
  ERR_INVALID_ARGUMENT = -4,
  ERR_INVALID_HANDLE = -5,
  ERR_FILE_NOT_FOUND = -6,
  ERR_TIMED_OUT = -7,
  ERR_FILE_TOO_BIG = -8,
  ERR_UNEXPECTED = -9,
  ERR_ACCESS_DENIED = -10,
  ERR_NOT_IMPLEMENTED = -11,
  ERR_CONNECTION_CLOSED = -100,
  ERR_CONNECTION_RESET = -101,
  ERR_CONNECTION_REFUSED = -102,
  ERR_CONNECTION_ABORTED = -103,
  ERR_CONNECTION_FAILED = -104,
  ERR_NAME_NOT_RESOLVED = -105,
  ERR_INTERNET_DISCONNECTED = -106,
  ERR_SSL_PROTOCOL_ERROR = -107,
  ERR_ADDRESS_INVALID = -108,
  ERR_ADDRESS_UNREACHABLE = -109,
  ERR_SSL_CLIENT_AUTH_CERT_NEEDED = -110,
  ERR_TUNNEL_CONNECTION_FAILED = -111,
  ERR_NO_SSL_VERSIONS_ENABLED = -112,
  ERR_SSL_VERSION_OR_CIPHER_MISMATCH = -113,
  ERR_SSL_RENEGOTIATION_REQUESTED = -114,
  ERR_CERT_COMMON_NAME_INVALID = -200,
  //ERR_CERT_BEGIN = ERR_CERT_COMMON_NAME_INVALID, // TODO
  ERR_CERT_DATE_INVALID = -201,
  ERR_CERT_AUTHORITY_INVALID = -202,
  ERR_CERT_CONTAINS_ERRORS = -203,
  ERR_CERT_NO_REVOCATION_MECHANISM = -204,
  ERR_CERT_UNABLE_TO_CHECK_REVOCATION = -205,
  ERR_CERT_REVOKED = -206,
  ERR_CERT_INVALID = -207,
  ERR_CERT_WEAK_SIGNATURE_ALGORITHM = -208,
  // -209 is available: was ERR_CERT_NOT_IN_DNS.
  ERR_CERT_NON_UNIQUE_NAME = -210,
  ERR_CERT_WEAK_KEY = -211,
  ERR_CERT_NAME_CONSTRAINT_VIOLATION = -212,
  ERR_CERT_VALIDITY_TOO_LONG = -213,
  //ERR_CERT_END = ERR_CERT_VALIDITY_TOO_LONG, // TODO
  ERR_INVALID_URL = -300,
  ERR_DISALLOWED_URL_SCHEME = -301,
  ERR_UNKNOWN_URL_SCHEME = -302,
  ERR_TOO_MANY_REDIRECTS = -310,
  ERR_UNSAFE_REDIRECT = -311,
  ERR_UNSAFE_PORT = -312,
  ERR_INVALID_RESPONSE = -320,
  ERR_INVALID_CHUNKED_ENCODING = -321,
  ERR_METHOD_NOT_SUPPORTED = -322,
  ERR_UNEXPECTED_PROXY_AUTH = -323,
  ERR_EMPTY_RESPONSE = -324,
  ERR_RESPONSE_HEADERS_TOO_BIG = -325,
  ERR_CACHE_MISS = -400,
  ERR_INSECURE_RESPONSE = -501,
}

#[repr(C)]
enum cef_process_id_t {
  PID_BROWSER,
  PID_RENDERER,
}

#[repr(C)]
enum cef_window_open_disposition_t {
  WOD_UNKNOWN,
  WOD_CURRENT_TAB,
  WOD_SINGLETON_TAB,
  WOD_NEW_FOREGROUND_TAB,
  WOD_NEW_BACKGROUND_TAB,
  WOD_NEW_POPUP,
  WOD_NEW_WINDOW,
  WOD_SAVE_TO_DISK,
  WOD_OFF_THE_RECORD,
  WOD_IGNORE_ACTION
}

#[repr(C)]
enum cef_state_t {
  STATE_DEFAULT = 0,
  STATE_ENABLED,
  STATE_DISABLED,
}

type cef_string_list_t = *mut c_void;

#[repr(C)]
struct cef_process_message_t {
    // TODO
}

#[repr(C)]
struct cef_view_delegate_t {
    base: cef_base_ref_counted_t,
    get_preferred_size: extern fn(self_: *mut cef_view_delegate_t, view: *mut cef_view_t) -> cef_size_t,
    get_minimum_size: extern fn(self_: *mut cef_view_delegate_t, view: *mut cef_view_t) -> cef_size_t,
    get_maximum_size: extern fn(self_: *mut cef_view_delegate_t, view: *mut cef_view_t) -> cef_size_t,
    get_height_for_width: extern fn(self_: *mut cef_view_delegate_t, view: *mut cef_view_t, width: i32) -> i32,
    on_parent_view_changed: extern fn(self_: *mut cef_view_delegate_t, view: *mut cef_view_t, added: i32,
        parent: *mut cef_view_t),
    on_child_view_changed: extern fn(self_: *mut cef_view_delegate_t, view: *mut cef_view_t, added: i32,
        child: *mut cef_view_t),
    on_focus: extern fn(self_: *mut cef_view_delegate_t, view: *mut cef_view_t),
    on_blur: extern fn(self_: *mut cef_view_delegate_t, view: *mut cef_view_t),
}

#[repr(C)]
struct cef_panel_delegate_t {
    base: cef_view_delegate_t,
}

#[repr(C)]
struct cef_window_delegate_t {
    base: cef_panel_delegate_t,
    on_window_created: extern fn(self_: *mut cef_window_delegate_t, window: *mut cef_window_t),
    on_window_destroyed: extern fn(self_: *mut cef_window_delegate_t, window: *mut cef_window_t),
    get_parent_window: extern fn(self_: *mut cef_window_delegate_t, window: *mut cef_window_t, is_mut: *mut i32,
        can_activate_menu: *mut i32) -> *mut cef_window_t,
    is_frameless: extern fn(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) -> i32,
    can_resize: extern fn(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) -> i32,
    can_maximize: extern fn(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) -> i32,
    can_minimize: extern fn(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) -> i32,
    can_close: extern fn(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) -> i32,
    on_accelerator: extern fn(self_: *mut cef_window_delegate_t, window: *mut cef_window_t, command_id: i32) -> i32,
    on_key_event: extern fn(self_: *mut cef_window_delegate_t, window: *mut cef_window_t, event: *const cef_key_event_t) -> i32
}

#[repr(C)]
struct cef_browser_settings_t {
    size: usize,
    windowless_frame_rate: i32,
    standard_font_family: cef_string_t,
    fixed_font_family: cef_string_t,
    serif_font_family: cef_string_t,
    sans_serif_font_family: cef_string_t,
    cursive_font_family: cef_string_t,
    fantasy_font_family: cef_string_t,
    default_font_size: i32,
    default_fixed_font_size: i32,
    minimum_font_size: i32,
    minimum_logical_font_size: i32,
    default_encoding: cef_string_t,
    remote_fonts: cef_state_t,
    javascript: cef_state_t,
    javascript_close_windows: cef_state_t,
    javascript_access_clipboard: cef_state_t,
    javascript_dom_paste: cef_state_t,
    plugins: cef_state_t,
    universal_access_from_file_urls: cef_state_t,
    file_access_from_file_urls: cef_state_t,
    web_security: cef_state_t,
    image_loading: cef_state_t,
    image_shrink_standalone_to_fit: cef_state_t,
    text_area_resize: cef_state_t,
    tab_to_links: cef_state_t,
    local_storage: cef_state_t,
    databases: cef_state_t,
    application_cache: cef_state_t,
    webgl: cef_state_t,
    background_color: cef_color_t,
    accept_language_list: cef_string_t,
}

#[repr(C)]
struct cef_panel_t {
    base: cef_view_t,
    as_window: extern fn(self_: *mut cef_panel_t) -> *mut cef_window_t,
    set_to_fill_layout: extern fn(self_: *mut cef_panel_t) -> *mut cef_fill_layout_t,
    set_to_box_layout: extern fn(self_: *mut cef_panel_t, settings: *const cef_box_layout_settings_t) -> *mut cef_box_layout_t,
    get_layout: extern fn(self_: *mut cef_panel_t) -> *mut cef_layout_t,
    layout: extern fn(self_: *mut cef_panel_t),
    add_child_view: extern fn(self_: *mut cef_panel_t, view: *mut cef_view_t),

  /*void(CEF_CALLBACK* add_child_view_at)(struct _cef_panel_t* self,
                                        struct _cef_view_t* view,
                                        int index);
  void(CEF_CALLBACK* reorder_child_view)(struct _cef_panel_t* self,
                                         struct _cef_view_t* view,
                                         int index);
  void(CEF_CALLBACK* remove_child_view)(struct _cef_panel_t* self,
                                        struct _cef_view_t* view);
  void(CEF_CALLBACK* remove_all_child_views)(struct _cef_panel_t* self);
  size_t(CEF_CALLBACK* get_child_view_count)(struct _cef_panel_t* self);
  struct _cef_view_t*(
      CEF_CALLBACK* get_child_view_at)(struct _cef_panel_t* self, int index);*/
}

#[repr(C)]
struct cef_window_t {
    base: cef_panel_t,
    show: extern fn(self_: *mut cef_window_t),

  /*void(CEF_CALLBACK* hide)(struct _cef_window_t* self);
  void(CEF_CALLBACK* center_window)(struct _cef_window_t* self,
                                    const cef_size_t* size);
  void(CEF_CALLBACK* close)(struct _cef_window_t* self);
  int(CEF_CALLBACK* is_closed)(struct _cef_window_t* self);
  void(CEF_CALLBACK* activate)(struct _cef_window_t* self);
  void(CEF_CALLBACK* deactivate)(struct _cef_window_t* self);
  int(CEF_CALLBACK* is_active)(struct _cef_window_t* self);
  void(CEF_CALLBACK* bring_to_top)(struct _cef_window_t* self);
  void(CEF_CALLBACK* set_always_on_top)(struct _cef_window_t* self, int on_top);
  int(CEF_CALLBACK* is_always_on_top)(struct _cef_window_t* self);
  void(CEF_CALLBACK* maximize)(struct _cef_window_t* self);
  void(CEF_CALLBACK* minimize)(struct _cef_window_t* self);
  void(CEF_CALLBACK* restore)(struct _cef_window_t* self);
  void(CEF_CALLBACK* set_fullscreen)(struct _cef_window_t* self,
                                     int fullscreen);
  int(CEF_CALLBACK* is_maximized)(struct _cef_window_t* self);
  int(CEF_CALLBACK* is_minimized)(struct _cef_window_t* self);
  int(CEF_CALLBACK* is_fullscreen)(struct _cef_window_t* self);
  void(CEF_CALLBACK* set_title)(struct _cef_window_t* self,
                                const cef_string_t* title);
  cef_string_userfree_t(CEF_CALLBACK* get_title)(struct _cef_window_t* self);
  void(CEF_CALLBACK* set_window_icon)(struct _cef_window_t* self,
                                      struct _cef_image_t* image);
  struct _cef_image_t*(CEF_CALLBACK* get_window_icon)(
      struct _cef_window_t* self);
  void(CEF_CALLBACK* set_window_app_icon)(struct _cef_window_t* self,
                                          struct _cef_image_t* image);
  struct _cef_image_t*(CEF_CALLBACK* get_window_app_icon)(
      struct _cef_window_t* self);
  void(CEF_CALLBACK* show_menu)(struct _cef_window_t* self,
                                struct _cef_menu_model_t* menu_model,
                                const cef_point_t* screen_point,
                                cef_menu_anchor_position_t anchor_position);
  void(CEF_CALLBACK* cancel_menu)(struct _cef_window_t* self);
  struct _cef_display_t*(CEF_CALLBACK* get_display)(struct _cef_window_t* self);
  cef_rect_t(CEF_CALLBACK* get_client_area_bounds_in_screen)(
      struct _cef_window_t* self);
  void(CEF_CALLBACK* set_draggable_regions)(
      struct _cef_window_t* self,
      size_t regionsCount,
      cef_draggable_region_t const* regions);
  cef_window_handle_t(CEF_CALLBACK* get_window_handle)(
      struct _cef_window_t* self);
  void(CEF_CALLBACK* send_key_press)(struct _cef_window_t* self,
                                     int key_code,
                                     uint32 event_flags);
  void(CEF_CALLBACK* send_mouse_move)(struct _cef_window_t* self,
                                      int screen_x,
                                      int screen_y);
  void(CEF_CALLBACK* send_mouse_events)(struct _cef_window_t* self,
                                        cef_mouse_button_type_t button,
                                        int mouse_down,
                                        int mouse_up);
  void(CEF_CALLBACK* set_accelerator)(struct _cef_window_t* self,
                                      int command_id,
                                      int key_code,
                                      int shift_pressed,
                                      int ctrl_pressed,
                                      int alt_pressed);
  void(CEF_CALLBACK* remove_accelerator)(struct _cef_window_t* self,
                                         int command_id);
  void(CEF_CALLBACK* remove_all_accelerators)(struct _cef_window_t* self);*/
}

#[repr(C)]
struct cef_fill_layout_t {
    // TODO
}

#[repr(C)]
struct cef_layout_t {
    // TODO
}

#[repr(C)]
struct cef_box_layout_settings_t {
    // TODO
}

#[repr(C)]
struct cef_box_layout_t {
    // TODO
}

#[repr(C)]
struct cef_view_t {
    // TODO
}

#[repr(C)]
struct cef_key_event_t {
    // TODO
}

#[repr(C)]
struct cef_size_t {
    // TODO
}

#[repr(C)]
struct cef_popup_features_t {
    // TODO
}

#[repr(C)]
struct cef_browser_view_t {
    base: cef_view_t,
    get_browser: extern fn(self_: *mut cef_browser_view_t),
    set_prefer_accelerators: extern fn(self_: *mut cef_browser_view_t, prefer_accelerators: i32),
}

#[repr(C)]
struct cef_browser_view_delegate_t {
    // TODO
}

#[repr(C)]
struct cef_request_context_t {
    // TODO
}

type cef_window_handle_t = u64;

#[repr(C)]
struct cef_window_info_t {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    parent_window: cef_window_handle_t,
    windowless_rendering_enabled: i32,
    shared_texture_enabled: i32,
    external_begin_frame_enabled: i32,
    window: cef_window_handle_t,
}

#[repr(C)]
struct cef_browser_t {
    // TODO
}

#[repr(C)]
struct cef_jsdialog_handler_t {
    // TODO
}

#[repr(C)]
struct cef_find_handler_t {
    // TODO
}

#[repr(C)]
struct cef_download_handler_t {
    // TODO
}

#[repr(C)]
struct cef_dialog_handler_t {
    // TODO
}

#[repr(C)]
struct cef_drag_handler_t {
    // TODO
}

#[repr(C)]
struct cef_focus_handler_t {
    // TODO
}

#[repr(C)]
struct cef_keyboard_handler_t {
    // TODO
}

#[repr(C)]
struct cef_render_handler_t {
    // TODO
}

#[repr(C)]
struct cef_context_menu_handler_t {
    // TODO
}

#[repr(C)]
struct cef_frame_t {
    // TODO
}

#[repr(C)]
struct cef_command_line_t {
    // TODO
}

#[repr(C)]
struct cef_scheme_registrar_t {
    // TODO
}

#[repr(C)]
struct cef_resource_bundle_handler_t {
    // TODO
}

#[repr(C)]
struct cef_display_handler_t {
    base: cef_base_ref_counted_t,
    on_address_change: extern fn(self_: *mut cef_display_handler_t, browser: *mut cef_browser_t, frame: *mut cef_frame_t,
        url: *const cef_string_t),
    on_title_change: extern fn (self_: *mut cef_display_handler_t, browser: *mut cef_browser_t, title: *const cef_string_t),
    on_favicon_urlchange: extern fn(self_: *mut cef_display_handler_t, browser: *mut cef_browser_t,
        icon_urls: cef_string_list_t),
    on_fullscreen_mode_change: extern fn(self_: *mut cef_display_handler_t, browser: *mut cef_browser_t, fullscreen: i32),
    on_tooltip: extern fn(self_: *mut cef_display_handler_t, browser: *mut cef_browser_t, text: *mut cef_string_t) -> i32,
    on_status_message: extern fn(self_: *mut cef_display_handler_t, browser: *mut cef_browser_t, value: *const cef_string_t),
    on_console_message: extern fn(self_: *mut cef_display_handler_t, browser: *mut cef_browser_t, level: cef_log_severity_t,
        message: *const cef_string_t, source: *const cef_string_t, line: i32) -> i32,
    on_auto_resize: extern fn(self_: *mut cef_display_handler_t, browser: *mut cef_browser_t, new_size: *const cef_size_t)
        -> i32,
    on_loading_progress_change: extern fn(self_: *mut cef_display_handler_t, browser: *mut cef_browser_t, progress: f64),
}

#[repr(C)]
struct cef_life_span_handler_t {
    base: cef_base_ref_counted_t,
    on_before_popup: extern fn(self_: *mut cef_life_span_handler_t, browser: *mut cef_browser_t, frame: *mut cef_frame_t,
        target_url: *const cef_string_t, target_frame_name: *const cef_string_t,
        target_disposition: cef_window_open_disposition_t, user_gesture: i32,
        popupFeatures: *const cef_popup_features_t, windowInfo: *mut cef_window_info_t, client: *mut *mut cef_client_t,
        settings: *mut cef_browser_settings_t, no_javascript_access: *mut i32) -> i32,
    on_after_created: extern fn (self_: *mut cef_life_span_handler_t, browser: *mut cef_browser_t),
    do_close: extern fn(self_: *mut cef_life_span_handler_t, browser: *mut cef_browser_t) -> i32,
      on_before_close: extern fn (self_: *mut cef_life_span_handler_t, browser: *mut cef_browser_t),
}

#[repr(C)]
struct cef_load_handler_t {
    base: cef_base_ref_counted_t,
    on_loading_state_change: extern fn(self_: *mut cef_load_handler_t, browser: *mut cef_browser_t, isLoading: i32,
        canGoBack: i32, canGoForward: i32),
    on_load_start: extern fn(self_: *mut cef_load_handler_t, browser: *mut cef_browser_t, frame: *mut cef_frame_t,
        transition_type: cef_transition_type_t),
    on_load_end: extern fn(self_: *mut cef_load_handler_t, browser: *mut cef_browser_t, frame: *mut cef_frame_t,
        httpStatusCode: i32),
    on_load_error: extern fn (self_: *mut cef_load_handler_t, browser: *mut cef_browser_t, frame: *mut cef_frame_t,
        errorCode: cef_errorcode_t, errorText: *const cef_string_t, failedUrl: *const cef_string_t),
}

#[repr(C)]
struct cef_client_t {
    base: cef_base_ref_counted_t,
    get_context_menu_handler: extern fn(self_: *mut cef_client_t) -> *mut cef_context_menu_handler_t,
    get_dialog_handler: extern fn(self_: *mut cef_client_t) -> *mut cef_dialog_handler_t,
    get_display_handler: extern fn (self_: *mut cef_client_t) -> *mut cef_display_handler_t,
    get_download_handler: extern fn(self_: *mut cef_client_t) -> *mut cef_download_handler_t,
    get_drag_handler: extern fn(self_: *mut cef_client_t) -> *mut cef_drag_handler_t,
    get_find_handler: extern fn(self_: *mut cef_client_t) -> *mut cef_find_handler_t,
    get_focus_handler: extern fn(self_: *mut cef_client_t) -> *mut cef_focus_handler_t,
    get_jsdialog_handler: extern fn(self_: *mut cef_client_t) -> *mut cef_jsdialog_handler_t,
    get_keyboard_handler: extern fn(self_: *mut cef_client_t) -> *mut cef_keyboard_handler_t,
    get_life_span_handler: extern fn (self_: *mut cef_client_t) -> *mut cef_life_span_handler_t,
    get_load_handler: extern fn (self_: *mut cef_client_t) -> *mut cef_load_handler_t,
    get_render_handler: extern fn(self_: *mut cef_client_t) -> *mut cef_render_handler_t,
    on_process_message_received: extern fn(self_: *mut cef_client_t, browser: *mut cef_browser_t,
        source_process: cef_process_id_t, message: *mut cef_process_message_t) -> i32,
}

#[repr(C)]
struct cef_browser_process_handler_t {
    base: cef_base_ref_counted_t,
    on_context_initialized: extern fn(_self: *const cef_browser_process_handler_t),
    on_before_child_process_launch: extern fn(_self: *const cef_browser_process_handler_t, command_line: *mut cef_command_line_t),
    on_render_process_thread_created: extern fn(_self: *const cef_browser_process_handler_t, extra_info: *mut cef_list_value_t),
    get_print_handler: extern fn(_self: *const cef_browser_process_handler_t) -> *mut cef_print_handler_t,
    on_schedule_message_pump_work: extern fn(_self: *const cef_browser_process_handler_t, delay_ms: i64),
}

#[repr(C)]
struct cef_print_handler_t {
    // TODO
}

#[repr(C)]
struct cef_list_value_t {
    // TODO
}

#[repr(C)]
struct cef_render_process_handler_t {
    // TODO
}

extern fn on_before_command_line_processing(_self: *mut cef_app_t, process_type: *const cef_string_t, command_line: *mut cef_command_line_t) {
}

extern fn on_register_custom_schemes(_self: *mut cef_app_t, registrar: *mut cef_scheme_registrar_t) {
}

extern fn get_resource_bundle_handler(_self: *mut cef_app_t) -> *mut cef_resource_bundle_handler_t {
    ptr::null_mut()
}

fn new_client() -> cef_client_t {
    extern fn get_context_menu_handler(self_: *mut cef_client_t) -> *mut cef_context_menu_handler_t {
        ptr::null_mut()
    }

    extern fn get_dialog_handler(self_: *mut cef_client_t) -> *mut cef_dialog_handler_t {
        ptr::null_mut()
    }

    extern fn get_display_handler(self_: *mut cef_client_t) -> *mut cef_display_handler_t {
        unimplemented!()
    }

    extern fn get_download_handler(self_: *mut cef_client_t) -> *mut cef_download_handler_t {
        ptr::null_mut()
    }

    extern fn get_drag_handler(self_: *mut cef_client_t) -> *mut cef_drag_handler_t {
        ptr::null_mut()
    }

    extern fn get_life_span_handler(self_: *mut cef_client_t) -> *mut cef_life_span_handler_t {
        unimplemented!()
    }

    extern fn get_load_handler(self_: *mut cef_client_t) -> *mut cef_load_handler_t {
        unimplemented!()
    }

    extern fn get_find_handler(self_: *mut cef_client_t) -> *mut cef_find_handler_t {
        ptr::null_mut()
    }

    extern fn get_focus_handler(self_: *mut cef_client_t) -> *mut cef_focus_handler_t {
        ptr::null_mut()
    }

    extern fn get_jsdialog_handler(self_: *mut cef_client_t) -> *mut cef_jsdialog_handler_t {
        ptr::null_mut()
    }

    extern fn get_keyboard_handler(self_: *mut cef_client_t) -> *mut cef_keyboard_handler_t {
        ptr::null_mut()
    }

    extern fn get_render_handler(self_: *mut cef_client_t) -> *mut cef_render_handler_t {
        ptr::null_mut()
    }

    extern fn on_process_message_received(self_: *mut cef_client_t, browser: *mut cef_browser_t,
        source_process: cef_process_id_t, message: *mut cef_process_message_t) -> i32 {
        0
    }

    let mut client: cef_client_t = unsafe { mem::zeroed() };
    client.base.size = mem::size_of::<cef_client_t>();
    client.get_context_menu_handler = get_context_menu_handler;
    client.get_dialog_handler = get_dialog_handler;
    client.get_display_handler = get_display_handler;
    client.get_download_handler = get_download_handler;
    client.get_drag_handler = get_drag_handler;
    client.get_find_handler = get_find_handler;
    client.get_focus_handler = get_focus_handler;
    client.get_jsdialog_handler = get_jsdialog_handler;
    client.get_keyboard_handler = get_keyboard_handler;
    client.get_life_span_handler = get_life_span_handler;
    client.get_load_handler = get_load_handler;
    client.get_render_handler = get_render_handler;
    client.on_process_message_received = on_process_message_received;

    client
}

fn new_delegate(browser_view: *mut cef_browser_view_t) -> cef_window_delegate_t {
    static mut BROWSER_VIEW: *mut cef_browser_view_t = ptr::null_mut();

    extern fn on_window_created(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) {
        unsafe {
            ((*window).base.add_child_view)(&mut (*window).base, &mut (*BROWSER_VIEW).base);
            ((*window).show)(window);
            println!("Window created");
            //(*BROWSER_VIEW).base.request_focus(&mut (*window).base);
        }
    }

    extern fn on_window_destroyed(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) {
    }

    extern fn get_parent_window(self_: *mut cef_window_delegate_t, window: *mut cef_window_t, is_mut: *mut i32,
        can_activate_menu: *mut i32) -> *mut cef_window_t
    {
        println!("get_parent_window");
        ptr::null_mut()
    }

    extern fn is_frameless(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) -> i32 {
        0
    }

    extern fn can_resize(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) -> i32 {
        0
    }

    extern fn can_maximize(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) -> i32 {
        0
    }

    extern fn can_minimize(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) -> i32 {
        0
    }

    extern fn can_close(self_: *mut cef_window_delegate_t, window: *mut cef_window_t) -> i32 {
        0
    }

    extern fn on_accelerator(self_: *mut cef_window_delegate_t, window: *mut cef_window_t, command_id: i32) -> i32 {
        0
    }

    extern fn on_key_event(self_: *mut cef_window_delegate_t, window: *mut cef_window_t, event: *const cef_key_event_t) -> i32 {
        0
    }

    unsafe {
        if BROWSER_VIEW.is_null() {
            BROWSER_VIEW = browser_view;
        }
    }

    let mut delegate: cef_window_delegate_t = unsafe { mem::zeroed() };
    delegate.base.base.base.size = mem::size_of::<cef_window_delegate_t>();
    delegate.on_window_created = on_window_created;
    delegate.on_window_destroyed = on_window_destroyed;
    delegate.get_parent_window = get_parent_window;
    delegate.is_frameless = is_frameless;
    delegate.can_resize = can_resize;
    delegate.can_maximize = can_maximize;
    delegate.can_minimize = can_minimize;
    delegate.can_close = can_close;
    delegate.on_accelerator = on_accelerator;
    delegate.on_key_event = on_key_event;
    delegate
}

extern fn get_browser_process_handler(_self: *mut cef_app_t) -> *mut cef_browser_process_handler_t {
    extern fn on_context_initialized(_self: *const cef_browser_process_handler_t) {
        let mut client = new_client();
        let mut browser_settings: cef_browser_settings_t = unsafe { mem::zeroed() };
        browser_settings.size = mem::size_of::<cef_browser_settings_t>();
        browser_settings.windowless_frame_rate = 30;
        unsafe {
            let browser_view = cef_browser_view_create(&mut client, &"https://www.google.ca".to_cef_string(),
                &mut browser_settings, ptr::null_mut(), ptr::null_mut());
            let mut delegate = new_delegate(browser_view);
            cef_window_create_top_level(&mut delegate);
        }
    }

    let mut handler: cef_browser_process_handler_t = unsafe { mem::zeroed() };
    handler.base.size = mem::size_of::<cef_app_t>();
    handler.on_context_initialized = on_context_initialized;
    Box::into_raw(Box::new(handler))
}

extern fn get_render_process_handler(_self: *mut cef_app_t) -> *mut cef_render_process_handler_t {
    ptr::null_mut()
}
