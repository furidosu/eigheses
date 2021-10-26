#[cfg(test)]
mod tests {
    use libass_sys::{
        ass_free_track, ass_get_available_font_providers, ass_library_done, ass_library_init,
        ass_read_file, ass_render_frame, ass_renderer_done, ass_renderer_init,
        ass_set_extract_fonts, ass_set_fonts, ass_set_frame_size, ASS_DefaultFontProvider,
        ASS_Image, ASS_Library, ASS_Renderer,
    };
    use std::{
        ffi::{CStr, CString},
        path::Path,
        ptr,
    };

    struct ImageT {
        width: i32,
        height: i32,
        stride: i32,
        buffer: Vec<u8>, // RGB24
    }

    fn write_png(fname: impl AsRef<Path>, img: &ImageT) {
        let fname = fname.as_ref();
        let fp = ::std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(fname)
            .expect(&format!(
                "PNG Error opening {} for writing!\n",
                fname.display()
            ));

        let png = image::codecs::png::PngEncoder::new(fp);
        png.encode(
            &img.buffer,
            img.width.try_into().unwrap(),
            img.height.try_into().unwrap(),
            image::ColorType::Rgb8,
        )
        .expect(&format!("PNG Error writing {}\n", fname.display()));
    }

    #[allow(non_upper_case_globals)]
    static mut ass_library: *mut ASS_Library = ptr::null_mut();
    #[allow(non_upper_case_globals)]
    static mut ass_renderer: *mut ASS_Renderer = ptr::null_mut();

    unsafe fn init(frame_w: i32, frame_h: i32) {
        ass_library = ass_library_init();
        if ass_library.is_null() {
            print!("ass_library_init failed!\n");
            std::process::exit(1);
        }

        ass_set_extract_fonts(ass_library, 1);

        ass_renderer = ass_renderer_init(ass_library);
        if ass_renderer.is_null() {
            print!("ass_renderer_init failed!\n");
            std::process::exit(1);
        }

        ass_set_frame_size(ass_renderer, frame_w, frame_h);
        ass_set_fonts(
            ass_renderer,
            ptr::null(),
            CStr::from_bytes_with_nul_unchecked(b"sans-serif\0").as_ptr(),
            ASS_DefaultFontProvider::ASS_FONTPROVIDER_AUTODETECT as i32,
            ptr::null(),
            1,
        );
    }

    fn gen_image(width: i32, height: i32) -> ImageT {
        let stride = width * 3;
        ImageT {
            width,
            height,
            stride,
            buffer: vec![63; (stride * height).try_into().unwrap()],
        }
    }

    macro_rules! _r {
        ($c:expr) => {
            u8::try_from($c >> 24).unwrap()
        };
    }
    macro_rules! _g {
        ($c:expr) => {
            u8::try_from(($c >> 16) & 0xFF).unwrap()
        };
    }
    macro_rules! _b {
        ($c:expr) => {
            u8::try_from(($c >> 8) & 0xFF).unwrap()
        };
    }
    macro_rules! _a {
        ($c:expr) => {
            u8::try_from($c & 0xFF).unwrap()
        };
    }

    fn blend_single(frame: &mut ImageT, img: &ASS_Image) {
        let opacity = 255 - _a!(img.color);
        let r = _r!(img.color);
        let g = _g!(img.color);
        let b = _b!(img.color);

        let mut src = img.bitmap;
        let mut dst = &mut frame.buffer[(img.dst_y * frame.stride + img.dst_x * 3) as usize..];

        unsafe {
            for _ in 0..img.h {
                for x in 0..img.w {
                    let src_x: u32 = (*src.add(x.try_into().unwrap())).into();
                    let k: u32 = src_x * opacity as u32 / 255;
                    let x: usize = x.try_into().unwrap();
                    // possible endianness problems
                    dst[x * 3] = ((k * b as u32 + (255 - k) * dst[x * 3] as u32) / 255) as u8;
                    dst[x * 3 + 1] =
                        ((k * g as u32 + (255 - k) * dst[x * 3 + 1] as u32) / 255) as u8;
                    dst[x * 3 + 2] =
                        ((k * r as u32 + (255 - k) * dst[x * 3 + 2] as u32) / 255) as u8;
                }
                src = src.add(img.stride.try_into().unwrap());
                dst = &mut dst[frame.stride.try_into().unwrap()..];
            }
        }
    }

    unsafe fn blend(frame: &mut ImageT, mut img: *const ASS_Image) {
        let mut cnt = 0;
        while !img.is_null() {
            let imgref = img.as_ref().unwrap();
            blend_single(frame, imgref);
            cnt += 1;
            img = (*img).next;
        }
        print!("{} images blended\n", cnt);
    }

    fn print_font_providers(ass_library_: *mut ASS_Library) {
        let font_provider_labels = [
            "None",
            "Autodetect",
            "CoreText",
            "Fontconfig",
            "DirectWrite",
        ];
        unsafe {
            let mut providers: *mut ASS_DefaultFontProvider = ptr::null_mut();
            let mut providers_size = 0;
            ass_get_available_font_providers(ass_library_, &mut providers, &mut providers_size);
            print!("test.c: Available font providers ({}): ", providers_size);
            for i in 0..providers_size {
                let separator = if i > 0 { ", " } else { "" };
                let p = *providers.add(i) as usize;
                print!("{}'{}'", separator, font_provider_labels[p]);
            }
            print!(".\n");
            libc::free(providers.cast());
        }
    }

    #[test]
    fn test1() {
        let frame_w = 1280;
        let frame_h = 720;

        unsafe {
            let imgfile = std::env::temp_dir().join("img.png");
            let subfile = CString::new("fixture/sub1.ass").unwrap();
            let tm = 0;

            print_font_providers(ass_library);

            init(frame_w, frame_h);
            let track = ass_read_file(ass_library, subfile.into_raw(), ptr::null_mut());
            if track.is_null() {
                print!("track init failed!\n");
                std::process::exit(1);
            }

            let img = ass_render_frame(ass_renderer, track, tm * 1000, ptr::null_mut());
            let mut frame = gen_image(frame_w, frame_h);
            blend(&mut frame, img);

            ass_free_track(track);
            ass_renderer_done(ass_renderer);
            ass_library_done(ass_library);

            write_png(&imgfile, &frame);
            print!("success writing render to {}\n", imgfile.display());
        }
    }

    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
