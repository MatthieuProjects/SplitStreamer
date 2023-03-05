use config::Config;
use gstreamer::prelude::*;
use splitstreamer::settings::ServerConfig;

const FALLBACK_PIPELINE: &str = "multifilesrc location=waiting.webm loop=true";
//const FALLBACK_PIPELINE: &str = "videotestsrc";
fn main() -> anyhow::Result<()> {
    // Initialize gstreamer
    gstreamer::init()?;

    let settings = Config::builder()
        .add_source(config::File::with_name("server"))
        .add_source(config::Environment::with_prefix("SPLITSTREAMER"))
        .build()?
        .try_deserialize::<ServerConfig>()?;
    let caps = gstreamer::Caps::builder("video/x-raw")
        .field("height", settings.resolution_h * settings.screen_h)
        .field("width", settings.resolution_w * settings.screen_w)
        .build();
    let pipeline = gstreamer::Pipeline::default();

    // This is our video source for now.
    let media_src: gstreamer::Element =
        gstreamer::parse_bin_from_description(FALLBACK_PIPELINE, true)
            .expect("Failed to initialize fallback pipeline")
            .upcast();
    // We decode the video
    let decodebin = gstreamer::ElementFactory::make("decodebin")
        .build()
        .unwrap();

    // Create a sink for audio
    let audio_sink = gstreamer::ElementFactory::make("autoaudiosink")
        .build()
        .unwrap();

    let encodebin = gstreamer::ElementFactory::make("x264enc")
        .property_from_str("speed-preset", "ultrafast")
        .property_from_str("tune", "zerolatency")
        .property("bitrate", 10000u32)
        .build()?;
    let payload = gstreamer::ElementFactory::make("rtph264pay").build()?;
    let conv = gstreamer::ElementFactory::make("videoconvert").build()?;
    let scl = gstreamer::ElementFactory::make("videoscale").build()?;

    // Create a sink for video
    let video_sink = gstreamer::ElementFactory::make("udpsink")
        .property_from_str("host", &settings.multicast_address)
        .property("port", settings.multicast_port as i32)
        .property("auto-multicast", true)
        .build()?;

    pipeline
        .add_many(&[
            &media_src,
            &audio_sink,
            &conv,
            &scl,
            &decodebin,
            &video_sink,
            &encodebin,
            &payload,
        ])
        .expect("failed to add element");

    media_src.link(&decodebin)?;
    conv.link(&scl)?;
    scl.link_filtered(&encodebin, &caps)?;
    encodebin.link(&payload)?;
    payload.link(&video_sink)?;

    decodebin.connect_pad_added(move |_, pad| {
        let caps = pad.current_caps().unwrap();
        let s = caps.structure(0).unwrap();

        let encodebin = conv.static_pad("sink").unwrap();
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
            MessageView::Eos(..) => {
                println!("EOS!");
                break;
            }
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
