use config::Config;
use gstreamer::glib;
use gstreamer::prelude::*;
use gstreamer::Pipeline;
use settings::ScreenConfig;

mod settings;

// constant pipelines definitions
// should be replaced!
const FALLBACK_PIPELINE: &str = "filesrc location=waiting.webm";

/// Builds a sink that outputs the image to multiple screens
fn build_output_sink(pipeline: &Pipeline, config: &ScreenConfig) -> gstreamer::Element {
    // Calculating total resolutions
    let total_width = config.screen_width * config.columns;
    let total_height = config.screen_height * config.lines;

    // Build caps wanted
    let caps = gstreamer::Caps::builder("video/x-raw")
        .field("height", total_height)
        .field("width", total_width)
        .build();

    let scale = gstreamer::ElementFactory::make("videoscale")
        .property("add-borders", true)
        .build()
        .expect("failed to create videoscale");

    let convert = gstreamer::ElementFactory::make("videoconvert")
        .build()
        .expect("failed to build videoconvert");

    let tee = gstreamer::ElementFactory::make("tee")
        .build()
        .expect("failed to create tee");
    let queue = gstreamer::ElementFactory::make("queue")
        .build()
        .expect("failed to create tee");

    pipeline
        .add_many(&[&tee, &scale, &convert, &queue])
        .expect("failed to add tee element");

    scale.link_filtered(&convert, &caps).unwrap();
    convert.link(&queue).unwrap();

    queue.link(&tee).unwrap();

    let mut index = 2;

    for line in 0..config.lines {
        for column in 0..config.columns {
            let top = line * total_height / config.lines;
            let bottom = total_height - (top + total_height / config.lines);

            let left = column * (total_width / config.columns);
            let right = total_width - (left + total_width / config.columns);

            let ip = format!("10.105.0.{}", index);
            index += 1;
            println!(
                "printing for screen {} at t: {} b: {} l: {}, r: {}",
                ip, top, bottom, left, right
            );

            let queue = gstreamer::ElementFactory::make("queue")
                .build()
                .expect("failed to create queue element");

            let videobox = gstreamer::ElementFactory::make("videobox")
                .property("right", right)
                .property("left", left)
                .property("top", top)
                .property("bottom", bottom)
                .build()
                .expect("failed to create videobox element");

            let encoder = gstreamer::ElementFactory::make("x264enc")
                .property_from_str("speed-preset", "ultrafast")
                .property_from_str("tune", "zerolatency")
                .property("bitrate", 3000u32)
                .build()
                .expect("failed to create enoder");

            let payloader = gstreamer::ElementFactory::make("rtph264pay")
                .build()
                .expect("failed to create payloader");

            let sink = gstreamer::ElementFactory::make("udpsink")
                .property("host", ip)
                .property("port", config.screen_port)
                .build()
                .expect("failed to create sink");

            pipeline
                .add_many(&[&queue, &videobox, &encoder, &payloader, &sink])
                .expect("failed to add elements to pipeline");

            // Link a pad from the tee to the queue
            tee.link_pads(Some("src_%u"), &queue, Some("sink"))
                .expect("failed to link elements");
            queue
                .link(&videobox)
                .expect("failed to link queue to videobox");
            videobox
                .link(&encoder)
                .expect("failed to videobox queue to encoder");
            encoder
                .link(&payloader)
                .expect("failed to link encoder to payloader");
            payloader
                .link(&sink)
                .expect("failed to link payloader to sink");
        }
    }
    scale
}
/*
fn create_pipeline() -> gstreamer::Pipeline {
    let pipeline = gstreamer::Pipeline::default();
    let user_src = gstreamer::parse_bin_from_description(MAIN_PIPELINE, true)
        .expect("Failed to initialize user pipeline")
        .upcast();
    let fallback_src = gstreamer::parse_bin_from_description(FALLBACK_PIPELINE, true)
        .expect("Failed to initialize fallback pipeline")
        .upcast();

    let output_sink = gstreamer::parse_bin_from_description(OUTPUT_PIPELINE, true)
        .expect("Failed to initialize output bin")
        .upcast();

    // Using a fallbackswitch to switch to the fallback stream when needed
    let fallbackswitch = gstreamer::ElementFactory::make("fallbackswitch")
        .property("timeout", gstreamer::ClockTime::SECOND)
        .build()
        .expect("Failed to initialize the fallbackswitch");

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
            &user_src,
            &fallback_src,
            &fallbackswitch,
            &decodebin,
            &videoconvert,
            &output_sink,
        ])
        .unwrap();

    /* The first pad requested will be automatically preferred */
    user_src
        .link_pads(Some("src"), &fallbackswitch, Some("sink_%u"))
        .unwrap();
    fallback_src
        .link_pads(Some("src"), &fallbackswitch, Some("sink_%u"))
        .unwrap();
    fallbackswitch
        .link_pads(Some("src"), &decodebin, Some("sink"))
        .unwrap();
    videoconvert
        .link_pads(Some("src"), &output_sink, Some("sink"))
        .unwrap();

    pipeline
}*/
fn start(config: &ScreenConfig) {
    let pipeline = gstreamer::Pipeline::default();

    // sample image
    let fallback_src: gstreamer::Element =
        gstreamer::parse_bin_from_description(FALLBACK_PIPELINE, true)
            .expect("Failed to initialize fallback pipeline")
            .upcast();
    let decodebin = gstreamer::ElementFactory::make("decodebin")
        .build()
        .unwrap();
    // audio sink for audio
    let audio_sink = gstreamer::ElementFactory::make("autoaudiosink")
        .build()
        .unwrap();

    let sink = build_output_sink(&pipeline, config);

    pipeline
        .add_many(&[&fallback_src, &audio_sink, &decodebin])
        .expect("failed to add element");

    fallback_src.link(&decodebin).expect("aaa");

    decodebin.connect_pad_added(move |_, pad| {
        let caps = pad.current_caps().unwrap();
        let s = caps.structure(0).unwrap();

        let sinkpad = sink.static_pad("sink").unwrap();
        let sinkpad2 = audio_sink.static_pad("sink").unwrap();

        println!("pad added to decodebin: {}", s.name());
        if s.name() == "video/x-raw" && !sinkpad.is_linked() {
            pad.link(&sinkpad).unwrap();
        } else if s.name() == "audio/x-raw" {
            pad.link(&sinkpad2).unwrap();
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
}

fn main() -> glib::ExitCode {
    gstreamer::init();
    // gstfallbackswitch::plugin_register_static().expect("Failed to register fallbackswitch plugin");

    let settings = Config::builder()
        // Add in `./Settings.toml`
        .add_source(config::File::with_name("config"))
        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        .add_source(config::Environment::with_prefix("APP"))
        .build()
        .unwrap()
        .try_deserialize::<ScreenConfig>()
        .unwrap();

    start(&settings);
    glib::ExitCode::SUCCESS
}
