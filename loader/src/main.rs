use std::ffi::c_void;
use std::process::exit;
use std::ptr::null;
use crate::preload::preload_info;

#[derive(Copy, Clone)]
pub struct WinePreloadInfo {
    addr: *const c_void,
    size: usize,
}

unsafe impl Sync for WinePreloadInfo {}

fn main() {
    init_reserved_areas();

    // if ((handle = try_dlopen(get_self_exe())) ||
    //     (handle = try_dlopen(argv[0])))
    // {
    //     void(*init_func)(int, char * *) = dlsym(handle, "__wine_main");
    //     if (init_func)
    //     init_func(argc, argv);
    //     fprintf(stderr, "wine: __wine_main function not found in ntdll.so\n");
    //     exit(1);
    // }
    //
    // fprintf(stderr, "wine: could not load ntdll.so: %s\n", dlerror());
    // pthread_detach(pthread_self());  /* force importing libpthread for OpenGL */
    exit(1);
}

#[cfg(not(feature = "have_wine_preloader"))]
mod preload {
    /*
    Not using the preloader on x86_64:
    Reserve the same areas as the preloader does, but using zero-fill sections
    (the only way to prevent system frameworks from using them, including allocations
    before main() runs).
    */
    use crate::WinePreloadInfo;
    use std::ffi::c_void;
    use std::ptr::null;

    #[unsafe(link_section = "WINE_RESERVE,WINE_RESERVE")]
    static __wine_reserve: [u8; 0x1fffff000] = [0; 0x1fffff000];

    #[unsafe(link_section = "WINE_TOP_DOWN,WINE_TOP_DOWN")]
    static __wine_top_down: [u8; 0x001ff0000] = [0; 0x001ff0000];

    pub(crate) static preload_info: [WinePreloadInfo; 3] = [
        /*         0x1000 -    0x200000000: low 8GB */
        WinePreloadInfo {
            addr: &__wine_reserve as *const u8 as *const c_void,
            size: size_of_val(&__wine_reserve),
        },
        /* 0x7ff000000000 - 0x7ff001ff0000: top-down allocations + virtual heap */
        WinePreloadInfo {
            addr: &__wine_top_down as *const u8 as *const c_void,
            size: size_of_val(&__wine_top_down),
        },
        /* end of list */
        WinePreloadInfo {
            addr: null(),
            size: 0,
        },
    ];
}

//FIXME: __attribute((visibility("default")))
#[allow(non_upper_case_globals)]
#[unsafe(no_mangle)]
static mut wine_main_preload_info: *const WinePreloadInfo = null();

#[cfg(not(feature = "have_wine_preloader"))]
fn init_reserved_areas() {
    unsafe { wine_main_preload_info = &preload_info as *const WinePreloadInfo }
    let mut preload_info_ptr = unsafe { wine_main_preload_info };
    loop {
        let info = unsafe { *preload_info_ptr };
        if info.size == 0 {
            break;
        }
        preload_info_ptr = unsafe { preload_info_ptr.add(1) };
        /* Match how the preloader maps reserved areas: */
        // mmap(wine_main_preload_info[i].addr, wine_main_preload_info[i].size, PROT_NONE, MAP_FIXED | MAP_NORESERVE | MAP_PRIVATE | MAP_ANON, - 1, 0);
    }
}

#[cfg(feature = "have_wine_preloader")]
fn init_reserved_areas() {
    // the preloader will set wine_main_preload_info
}
