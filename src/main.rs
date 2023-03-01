use gio::glib::ExitCode;
use gio::prelude::*;

use gstreamer::glib;
use gstreamer::prelude::*;

mod config;

const MAIN_PIPELINE: &str = "udpsrc port=8089 ! rtph264depay ! 'video/x-raw,width=1280,height=720'";
const FALLBACK_PIPELINE: &str =
    "videotestsrc is-live=true pattern=snow ! 'video/x-raw,width=1280,height=720'";
const OUTPIT_PIEPELINE: &str = "x264enc ! rtph264pay ! udpsink host=10.81.0.2 port=5001";

//const MAIN_PIPELINE: &str = "videotestsrc is-live=true pattern=ball ! x264enc tune=zerolatency";
//const FALLBACK_PIPELINE: &str = "videotestsrc is-live=true pattern=snow ! x264enc tune=zerolatency";

fn create_pipeline() -> gstreamer::Pipeline {
    let pipeline = gstreamer::Pipeline::default();

    let video_src = gstreamer::parse_bin_from_description(MAIN_PIPELINE, true)
        .unwrap()
        .upcast();
    let fallback_video_src = gstreamer::parse_bin_from_description(FALLBACK_PIPELINE, true)
        .unwrap()
        .upcast();
    let video_sink = gstreamer::parse_bin_from_description(OUTPIT_PIEPELINE, true)
        .unwrap()
        .upcast();

    let fallbackswitch = gstreamer::ElementFactory::make("fallbackswitch")
        .property("timeout", gstreamer::ClockTime::SECOND)
        .build()
        .unwrap();

    let decodebin = gstreamer::ElementFactory::make("decodebin")
        .build()
        .unwrap();
    let videoconvert = gstreamer::ElementFactory::make("videoconvert")
        .build()
        .unwrap();

    let videoconvert_clone = videoconvert.clone();
    decodebin.connect_pad_added(move |_, pad| {
        let caps = pad.current_caps().unwrap();
        let s = caps.structure(0).unwrap();

        let sinkpad = videoconvert_clone.static_pad("sink").unwrap();

        if s.name() == "video/x-raw" && !sinkpad.is_linked() {
            pad.link(&sinkpad).unwrap();
        }
    });

    pipeline
        .add_many(&[
            &video_src,
            &fallback_video_src,
            &fallbackswitch,
            &decodebin,
            &videoconvert,
            &video_sink,
        ])
        .unwrap();

    /* The first pad requested will be automatically preferred */
    video_src
        .link_pads(Some("src"), &fallbackswitch, Some("sink_%u"))
        .unwrap();
    fallback_video_src
        .link_pads(Some("src"), &fallbackswitch, Some("sink_%u"))
        .unwrap();
    fallbackswitch
        .link_pads(Some("src"), &decodebin, Some("sink"))
        .unwrap();
    videoconvert
        .link_pads(Some("src"), &video_sink, Some("sink"))
        .unwrap();

    pipeline
}

fn start() {
    let pipeline = create_pipeline();

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
}

fn main() -> glib::ExitCode {
    gstreamer::init().unwrap();

    gstfallbackswitch::plugin_register_static().expect("Failed to register fallbackswitch plugin");
    start();
    return ExitCode::SUCCESS;
}
