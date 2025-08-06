use std::cell::RefCell;
use std::error::Error;
use std::ffi::OsString;
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

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

    let video_path = RefCell::new(String::from(""));
    let mut video_label = nwg::Label::default();
    let mut open_video_button = nwg::Button::default();

    let sub_path = RefCell::new(String::from(""));
    let mut sub_label = nwg::Label::default();
    let mut open_sub_button = nwg::Button::default();

    let combine_sub_button = Rc::new(RefCell::new(nwg::Button::default()));

    nwg::Window::builder()
        .size((600, 100))
        .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
        .title("KWH Tool")
        .build(&mut window)?;

    nwg::Button::builder()
        .parent(&window)
        .text("Open Video")
        .build(&mut open_video_button)?;

    nwg::Label::builder()
        .parent(&window)
        .text("Not selected")
        .build(&mut video_label)?;

    nwg::Button::builder()
        .parent(&window)
        .text("Open Subtitle")
        .build(&mut open_sub_button)?;

    nwg::Label::builder()
        .parent(&window)
        .text("Not selected")
        .build(&mut sub_label)?;

    nwg::Button::builder()
        .parent(&window)
        .text("Render")
        .build(&mut combine_sub_button.borrow_mut())?;

    let grid = nwg::GridLayout::default();
    nwg::GridLayout::builder()
        .parent(&window)
        .spacing(1)
        .child(0, 0, &open_video_button)
        .child(1, 0, &video_label)
        .child(0, 1, &open_sub_button)
        .child(1, 1, &sub_label)
        .child_item(nwg::GridLayoutItem::new(&*(combine_sub_button).borrow(), 0, 2, 2, 1))
        .build(&grid)?;

    let window = Rc::new(window);
    let events_window = window.clone();

    let handler_sub_button = Rc::clone(&combine_sub_button);
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

                        let tx = tx.clone();
                        thread::spawn(move || {
                            let mut ffmpeg_process = Command::new("ffmpeg")
                                .arg("-y")
                                .arg("-i").arg(video_path)
                                .arg("-vf").arg(format!("ass=\'{}\'", sub_path.replace('\\', "/")).replace(':', "\\:"))
                                .arg("-crf").arg("18")
                                .arg("-preset").arg("slow")
                                .arg("-movflags").arg("+faststart")
                                .arg("-c:v").arg("libx264")
                                .arg("-c:a").arg("copy")
                                .arg(saved_path)
                                .spawn().unwrap();

                            ffmpeg_process.wait().unwrap();
                            tx.send(1).unwrap();
                        });
                    }

                }
            }
            _ => {}
        }
    });

    let sub_button = Rc::clone(&combine_sub_button);
    nwg::dispatch_thread_events_with_callback(move || {
        if rx.try_recv().is_ok() {
            sub_button.borrow().set_text("Render");
            sub_button.borrow().set_enabled(true);
        }
    });
    nwg::unbind_event_handler(&handler);

    Ok(())
}

fn trim_path(path: OsString) -> String {
    let path = path.to_str().unwrap();
    let mut trimmed_path_str = "...".to_string();
    trimmed_path_str.push_str(&path[path.len() - 40..].to_string());

    trimmed_path_str
}
