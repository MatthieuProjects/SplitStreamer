use config::Config;
use gstreamer::prelude::*;
use server::splitscreen_bin::build_spliscreen_bin;
use shared::config::ServerConfig;

//const FALLBACK_PIPELINE: &str = "videotestsrc";
fn main() -> anyhow::Result<()> {
    // Initialize gstreamer
    gstreamer::init()?;

    let file = "waiting.webm";

    let settings = Config::builder()
        .add_source(config::File::with_name("server"))
        .add_source(config::Environment::with_prefix("SPLITSTREAMER"))
        .build()?
        .try_deserialize::<ServerConfig>()?;

    let pipeline = gstreamer::Pipeline::default();

    // This is our video source for now.
    let media_src: gstreamer::Element = gstreamer::ElementFactory::make("filesrc")
        .property("location", file)
        .build()
        .unwrap();

    // We decode the video
    let decodebin = gstreamer::ElementFactory::make("decodebin")
        .build()
        .unwrap();

    // Create a sink for audio
    let audio_sink = gstreamer::ElementFactory::make("autoaudiosink")
        .build()
        .unwrap();

    let splitscreen_bin = build_spliscreen_bin(&settings)?;
    pipeline.add(&splitscreen_bin)?;
    pipeline
        .add_many(&[&media_src, &audio_sink, &decodebin])
        .expect("failed to add element");

    media_src.link(&decodebin)?;

    decodebin.connect_pad_added(move |_, pad| {
        let caps = pad.current_caps().unwrap();
        let s = caps.structure(0).unwrap();

        let encodebin = splitscreen_bin.static_pad("sink").unwrap();
        let audio_sink_pad = audio_sink.static_pad("sink").unwrap();

        println!("pad added to decodebin: {}", s.name());
        if s.name() == "video/x-raw" && !encodebin.is_linked() {
            pad.link(&encodebin).unwrap();
        } else if s.name() == "audio/x-raw" && audio_sink_pad.is_linked() {
            pad.link(&audio_sink_pad).unwrap();
        }
    });

    let bus = pipeline.bus().unwrap();
    pipeline.set_state(gstreamer::State::Playing).unwrap();

    for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
        use gstreamer::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                break;
            }
            _ => (),
        }
    }

    pipeline
        .set_state(gstreamer::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");

    Ok(())
}
