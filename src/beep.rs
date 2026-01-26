use rodio::{Decoder, OutputStream, Sink, Source};
use std::io::Cursor;
const BEEP_SOUND: &[u8] = include_bytes!("../assets/ah.mp3");
pub struct BeepController {
    sink: Sink,
    _handle: OutputStream,
}
impl BeepController {
    pub fn new() -> anyhow::Result<Self> {
        let stream_handler = rodio::OutputStreamBuilder::open_default_stream()?;
        let sink = Sink::connect_new(stream_handler.mixer());
        sink.set_volume(0.3);

        Ok(Self {
            _handle: stream_handler,
            sink: sink,
        })
    }
    pub fn start(&mut self) {
        let cursor = Cursor::new(BEEP_SOUND);
        match Decoder::new(cursor) {
            Ok(src) => {
                self.sink.clear();
                self.sink.append(src.repeat_infinite());
                self.sink.play();
            }
            Err(err) => {
                eprintln!("Error sinking {err}");
            }
        }
    }
    pub fn stop(&self) {
        self.sink.clear();
    }
}
