use config::Config;
use gstreamer::{
    prelude::{ElementExtManual, GstBinExtManual},
    traits::{ElementExt, GstObjectExt, PadExt},
    Caps, Pipeline,
};
use shared::{build_videoconvertscale, config::ClientConfig};

/**
 * This program listens for a stream on the network.
 * Once it receives the stream, it cuts it according to the screen position in the config file
 */
fn main() -> anyhow::Result<()> {
    // Initializing gstreamer
    gstreamer::init()?;

    // Loading configuration.
    let settings = Config::builder()
        .add_source(config::File::with_name("client"))
        .build()?;
    // We deserialize our config file according to the ClientConfig struct.
    let client_config = settings.try_deserialize::<ClientConfig>()?;

    // We are expecting this signal from the streaming server.
    let rtp_caps = Caps::builder("application/x-rtp")
        .field("clock-rate", 90000)
        .build();

    // Initializing the GStreamer pipeline described like this:
    // udpsrc -> rtpjitterbuffer -> rtph264depay -> (avdec_h264) -> videoconvertscale -> videobox -> videoconvertscale -> autovideosink
    let pipeline = Pipeline::default();

    // Listening for the packets in the multicast group.
    let udp_source = gstreamer::ElementFactory::make("udpsrc")
        .property_from_str("multicast-group", &client_config.multicast_address)
        .property("auto-multicast", true)
        .property("port", client_config.multicast_port as i32)
        .build()?;
    let jitter_buffer = gstreamer::ElementFactory::make("rtpjitterbuffer")
        .property_from_str("mode", "slave")
        .build()?;
    let rtp_depayloader = gstreamer::ElementFactory::make("rtph264depay").build()?;
    let decoder = gstreamer::ElementFactory::make("decodebin").build()?;
    let videoconvertscale0 = build_videoconvertscale()?;
    let videobox = gstreamer::ElementFactory::make("videobox")
        .property("right", client_config.video_box.right as i32)
        .property("left", client_config.video_box.left as i32)
        .property("top", client_config.video_box.top as i32)
        .property("bottom", client_config.video_box.bottom as i32)
        .build()
        .expect("failed to create videobox element");
    let videoconvertscale1 = build_videoconvertscale()?;
    let sink = gstreamer::ElementFactory::make("autovideosink").build()?;

    // Add the elements to the pipeline
    pipeline.add_many(&[
        &udp_source,
        &jitter_buffer,
        &rtp_depayloader,
        &decoder,
        &videobox,
        &sink,
    ])?;
    pipeline.add_many(&[&videoconvertscale0, &videoconvertscale1])?;

    // Linking the pipeline.
    udp_source.link_filtered(&jitter_buffer, &rtp_caps)?;
    jitter_buffer.link(&rtp_depayloader)?;
    rtp_depayloader.link(&decoder)?;

    videoconvertscale0.link(&videobox)?;
    videobox.link(&videoconvertscale1)?;
    videoconvertscale1.link(&sink)?;

    // the decoder is a bin with dynamic pads, so we need to add an event.
    decoder.connect_pad_added(move |_, pad| {
        let caps = pad.current_caps().unwrap();
        let s = caps.structure(0).unwrap();

        // This is the pad we want to link out decoder to
        let video_sink_pad = videoconvertscale0.static_pad("sink").unwrap();

        // we are only interesed by the video streams that are unlinked.
        if s.name() == "video/x-raw" && !video_sink_pad.is_linked() {
            pad.link(&video_sink_pad).unwrap();
        }
    });

    // Starting the bus.
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

    // this pipeline shouldn't end except when an error is raised.

    pipeline.set_state(gstreamer::State::Null)?;

    Ok(())
}
