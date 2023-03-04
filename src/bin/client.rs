// Program that listens for a multicast stream
// decodes it, cuts it and display it

use config::Config;
use gstreamer::{
    prelude::{ElementExtManual, GstBinExtManual},
    traits::{ElementExt, GstObjectExt, PadExt},
    Caps, Fraction, Pipeline,
};
use splitstreamer::settings::ClientConfigFile;

const CAPS: &str = "application/x-rtp";

fn main() -> anyhow::Result<()> {
    // Initialize gstreamer
    gstreamer::init()?;

    let settings = Config::builder()
        .add_source(config::File::with_name("client"))
        .add_source(config::Environment::with_prefix("SPLITSTREAMER"))
        .build()?;

    let caps = Caps::builder("application/x-rtp")
        .field("clock-rate", 90000)
        .build();

    let caps2 = Caps::builder("video/x-raw")
        .field("height", 1080)
        .field("width", 1920)
        .build();

    let self_id: usize = settings.get("id")?;
    let client_configs = settings.try_deserialize::<ClientConfigFile>()?;

    let client_config = client_configs
        .configs
        .get(self_id)
        .ok_or_else(|| anyhow::anyhow!("self does not exist"))?;

    let pipeline = Pipeline::default();

    // We need to build a pipeline that receives a udp stream
    // handles the jitter
    let udp_source = gstreamer::ElementFactory::make("udpsrc")
        .property_from_str("multicast-group", &client_config.multicast_address)
        .property("auto-multicast", true)
        .property("port", client_config.multicast_port as i32)
        .build()?;

    let depayloader = gstreamer::ElementFactory::make("rtph264depay").build()?;
    // we need to be able to handle any codec
    let decodebin = gstreamer::ElementFactory::make("avdec_h264").build()?;
    let videobox = gstreamer::ElementFactory::make("videobox")
        .property("right", client_config.video_box.right as i32)
        .property("left", client_config.video_box.left as i32)
        .property("top", client_config.video_box.top as i32)
        .property("bottom", client_config.video_box.bottom as i32)
        .build()
        .expect("failed to create videobox element");
    // we output the data
    let auto_video_sink = gstreamer::ElementFactory::make("autovideosink").build()?;
    let convert = gstreamer::ElementFactory::make("videoconvert").build()?;
    pipeline.add_many(&[
        &udp_source,
        &decodebin,
        &depayloader,
        &convert,
        &auto_video_sink,
        &videobox,
    ])?;

    // udp_source ! deplayloader ! decodebin ! videobox ! convert ! auto_video_sink

    udp_source.link_filtered(&depayloader, &caps)?;
    depayloader.link(&decodebin)?;
    decodebin.link(&convert)?;
    convert.link_filtered(&videobox, &caps2)?;
    videobox.link(&auto_video_sink)?;

    /*decodebin.connect_pad_added(move |_, pad| {
        let caps = pad.current_caps().unwrap();
        let s = caps.structure(0).unwrap();

        let video_sink_pad = auto_video_sink.static_pad("sink").unwrap();

        println!("pad added to decodebin: {}", s.name());
        if s.name() == "video/x-raw" && !video_sink_pad.is_linked() {
            pad.link(&video_sink_pad).unwrap();
        }
    });*/

    let bus = pipeline.bus().expect("failed to get bus");
    pipeline.set_state(gstreamer::State::Playing)?;

    // Start the pipeline
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

    pipeline.set_state(gstreamer::State::Null)?;

    Ok(())
}
