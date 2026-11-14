fn main() {
    let _session = webca_windows::MediaSession::start().expect("media session");
    for camera in webca_windows::list_cameras().expect("cameras") {
        println!("{}\n  {}", camera.info.name, camera.device_path);
    }
}
