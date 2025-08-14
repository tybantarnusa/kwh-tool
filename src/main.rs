// #![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use mp4::Mp4Reader;
use nwg::stretch::geometry::Rect;
use nwg::stretch::style::{Dimension, FlexDirection};
use winapi::um::winbase::CREATE_NO_WINDOW;

static HEADER_BITMAP: &[u8] = include_bytes!("../assets/header.png");

enum FfmpegProgress {
    Progress(u32),
    Done,
}

fn main() -> Result<(), Box<dyn Error>> {
    nwg::init().expect("Failed to init Native Windows GUI");
    let (tx, rx) = mpsc::channel();

    let mut font = nwg::Font::default();

    nwg::Font::builder()
        .family("Calibri")
        .size(18)
        .build(&mut font)?;

    nwg::Font::set_global_default(Some(font));

    let mut window = nwg::Window::default();
    let mut container = nwg::Frame::default();
    let container_grid = nwg::FlexboxLayout::default();

    let header_bitmap = nwg::Bitmap::from_bin(HEADER_BITMAP)?;
    let mut header_frame = nwg::ImageFrame::default();

    let video_path = Rc::new(RefCell::new(String::from("")));
    let mut video_label = nwg::Label::default();
    let mut open_video_button = nwg::Button::default();

    let sub_path = RefCell::new(String::from(""));
    let mut sub_label = nwg::Label::default();
    let mut open_sub_button = nwg::Button::default();

    let combine_sub_button = Rc::new(RefCell::new(nwg::Button::default()));

    let video_duration = Rc::new(RefCell::new(Duration::default()));

    let progress_bar = Rc::new(RefCell::new(nwg::ProgressBar::default()));

    nwg::Window::builder()
        .size((600, 270))
        .flags(nwg::WindowFlags::WINDOW)
        .title("KWH Tool")
        .build(&mut window)?;

    nwg::ImageFrame::builder()
        .parent(&window)
        .bitmap(Some(&header_bitmap))
        .build(&mut header_frame)?;

    nwg::Frame::builder()
        .parent(&window)
        .flags(nwg::FrameFlags::VISIBLE)
        .build(&mut container)?;

    nwg::ImageFrame::builder()
        .parent(&window)
        .bitmap(Some(&header_bitmap))
        .build(&mut header_frame)?;

    nwg::FlexboxLayout::builder()
        .parent(&window)
        .flex_direction(FlexDirection::Column)
        .padding(Rect {
            start: Dimension::Points(0.0),
            end: Dimension::Points(0.0),
            top: Dimension::Points(0.0),
            bottom: Dimension::Points(0.0),
        })
        .child(&header_frame)
        .child_flex_grow(1.0)
        .child(&container)
        .child_flex_grow(1.0)
        .build(&container_grid)?;

    nwg::Button::builder()
        .parent(&container)
        .text("Open Video")
        .build(&mut open_video_button)?;

    nwg::Label::builder()
        .parent(&container)
        .text("Not selected")
        .build(&mut video_label)?;

    nwg::Button::builder()
        .parent(&container)
        .text("Open Subtitle")
        .build(&mut open_sub_button)?;

    nwg::Label::builder()
        .parent(&container)
        .text("Not selected")
        .build(&mut sub_label)?;

    nwg::Button::builder()
        .parent(&container)
        .text("Render")
        .enabled(false)
        .build(&mut combine_sub_button.borrow_mut())?;

    nwg::ProgressBar::builder()
        .parent(&container)
        .build(&mut progress_bar.borrow_mut())?;
    progress_bar.borrow().set_visible(false);

    let grid = nwg::GridLayout::default();
    nwg::GridLayout::builder()
        .parent(&container)
        .spacing(1)
        .child(0, 0, &open_video_button)
        .child(1, 0, &video_label)
        .child(0, 1, &open_sub_button)
        .child(1, 1, &sub_label)
        .child_item(nwg::GridLayoutItem::new(&*(combine_sub_button).borrow(), 0, 2, 2, 1))
        .child_item(nwg::GridLayoutItem::new(&*(progress_bar).borrow(), 0, 3, 2, 1))
        .build(&grid)?;

    window.set_visible(true);

    let window = Rc::new(window);
    let events_window = window.clone();

    let handler_sub_button = Rc::clone(&combine_sub_button);
    let duration_clone = Rc::clone(&video_duration);
    let progress_bar_clone = Rc::clone(&progress_bar);
    let handler = nwg::full_bind_event_handler(&window.handle, move |evt, _evt_data, handle| {
        use nwg::Event as E;

        match evt {
            E::OnWindowClose => {
                if &handle == &events_window as &nwg::Window {
                    nwg::stop_thread_dispatch();
                }
            }
            E::OnButtonClick => {
                if &handle == &open_video_button.handle {
                    let mut video_file = Default::default();

                    nwg::FileDialog::builder()
                        .title("Select a video file")
                        .action(nwg::FileDialogAction::Open)
                        .filters("Video Files(*.mp4)")
                        .build(&mut video_file)
                        .unwrap();

                    if video_file.run(Some(&open_video_button)) {
                        if let Ok(path) = video_file.get_selected_item() {
                            *video_path.borrow_mut() = path.to_str().unwrap().to_string();
                            video_label.set_text(trim_path(path).as_str());
                            handler_sub_button.borrow_mut().set_enabled(true);

                            let path = Rc::clone(&video_path);
                            let mp4_file = Box::new(File::open(&*path.borrow()).unwrap());
                            let size = mp4_file.metadata().unwrap().len();
                            let reader = BufReader::new(mp4_file);

                            let mp4_header = Mp4Reader::read_header(reader, size).unwrap();
                            let duration = mp4_header.duration();
                            progress_bar_clone.borrow().set_range(0..duration.as_millis() as u32);
                            *duration_clone.borrow_mut() = duration;
                        }
                    }
                } else if &handle == &open_sub_button.handle {
                    let mut sub_file = Default::default();

                    nwg::FileDialog::builder()
                        .title("Select a subtitle file")
                        .action(nwg::FileDialogAction::Open)
                        .filters("Subtitle ASS Files(*.ass)")
                        .build(&mut sub_file)
                        .unwrap();

                    if sub_file.run(Some(&open_sub_button)) {
                        if let Ok(path) = sub_file.get_selected_item() {
                            *sub_path.borrow_mut() = path.to_str().unwrap().to_string();
                            sub_label.set_text(trim_path(path).as_str());
                        }
                    }
                } else if &handle == &(handler_sub_button.borrow()).handle {
                    let video_path = video_path.borrow().to_string();
                    let sub_path = sub_path.borrow().to_string();

                    let mut saved_file = Default::default();
                    let mut saved_path = String::from("");
                    nwg::FileDialog::builder()
                        .title("Save subbed file to")
                        .action(nwg::FileDialogAction::Save)
                        .filters("Video Files(*.mp4)")
                        .build(&mut saved_file)
                        .unwrap();

                    if saved_file.run(Some(&(*handler_sub_button.borrow()))) {
                        if let Ok(path) = saved_file.get_selected_item() {
                            saved_path = path.into_string().unwrap();
                            if !saved_path.ends_with(".mp4") {
                                saved_path.push_str(".mp4");
                            }
                        }

                        handler_sub_button.borrow().set_text("Rendering...");
                        handler_sub_button.borrow().set_enabled(false);

                        progress_bar_clone.borrow().set_visible(true);

                        let tx = tx.clone();
                        thread::spawn(move || {
                            let mut ffmpeg_process = Command::new("ffmpeg")
                                .arg("-y")
                                .arg("-progress").arg("pipe:2")
                                .arg("-i").arg(video_path)
                                .arg("-vf").arg(format!("ass=\'{}\'", sub_path.replace('\\', "/")).replace(':', "\\:"))
                                .arg("-crf").arg("18")
                                .arg("-preset").arg("slow")
                                .arg("-movflags").arg("+faststart")
                                .arg("-c:v").arg("libx264")
                                .arg("-c:a").arg("copy")
                                .arg(saved_path)
                                .creation_flags(CREATE_NO_WINDOW)
                                .stderr(Stdio::piped())
                                .spawn().unwrap();

                            let stderr = ffmpeg_process.stderr.take().unwrap();
                            let reader = BufReader::new(stderr);

                            for line in reader.lines() {
                                if let Ok(line) = line {
                                    if line.contains("out_time_ms") {
                                        let ffmpeg_time = &line[12..];
                                        let ffmpeg_time_num = ffmpeg_time.parse::<u32>().unwrap();

                                        tx.send(FfmpegProgress::Progress(ffmpeg_time_num/1000)).unwrap();
                                    }
                                }
                            }

                            ffmpeg_process.wait().unwrap();
                            tx.send(FfmpegProgress::Done).unwrap();
                        });
                    }

                }
            }
            _ => {}
        }
    });

    let sub_button = Rc::clone(&combine_sub_button);
    let progress_bar_clone = Rc::clone(&progress_bar);
    nwg::dispatch_thread_events_with_callback(move || {
        if let Ok(progress) = rx.try_recv() {
            match progress {
                FfmpegProgress::Progress(ffmpeg_time) => {
                    progress_bar_clone.borrow().set_pos(ffmpeg_time);
                }
                FfmpegProgress::Done => {
                    sub_button.borrow().set_text("Render");
                    sub_button.borrow().set_enabled(true);
                    progress_bar_clone.borrow().set_pos(0);
                    progress_bar_clone.borrow().set_visible(false);
                }
            }
        }
    });
    nwg::unbind_event_handler(&handler);

    Ok(())
}

fn trim_path(path: OsString) -> String {
    if path.len() < 40 {
        return path.to_str().unwrap().to_string();
    }

    let path = path.to_str().unwrap();
    let mut trimmed_path_str = "...".to_string();
    trimmed_path_str.push_str(&path[path.len() - 40..].to_string());

    trimmed_path_str
}
