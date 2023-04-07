use async_tungstenite::tungstenite::Message as WsMessage;
use config::Config;
use futures_util::SinkExt;
use futures_util::StreamExt;
use shared::config::ServerConfig;
use tokio::select;

use server::webrtc::App;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize GStreamer first
    gstreamer::init()?;
    gstfallbackswitch::plugin_register_static().expect("Failed to register fallbackswitch plugin");

    let settings = Config::builder()
        .add_source(config::File::with_name("server"))
        .add_source(config::Environment::with_prefix("SPLITSTREAMER"))
        .build()?
        .try_deserialize::<ServerConfig>()?;

    // Connect to the given server
    let (ws, _) = async_tungstenite::tokio::connect_async(&settings.signaling_server).await?;

    // Split the websocket into the Sink and Stream
    let (mut ws_sink, ws_stream) = ws.split();
    // Fuse the Stream, required for the select macro
    let mut ws_stream = ws_stream.fuse();

    // Create our application state
    let (app, send_gst_msg_rx, send_ws_msg_rx) = App::new(settings)?;
    let mut send_gst_msg_rx = send_gst_msg_rx.fuse();
    let mut send_ws_msg_rx = send_ws_msg_rx.fuse();
    
    // And now let's start our message loop
    loop {
        let ws_msg = select! {
            // Handle the WebSocket messages here
            ws_msg = ws_stream.next() => {
                match ws_msg.unwrap()? {
                    WsMessage::Close(_) => {
                        println!("peer disconnected");
                        break
                    },
                    WsMessage::Ping(data) => Some(WsMessage::Pong(data)),
                    WsMessage::Pong(_) => None,
                    WsMessage::Binary(_) => None,
                    WsMessage::Text(text) => {
                        if let Err(err) = app.handle_websocket_message(&text) {
                            println!("Failed to parse message: {err}");
                        }
                        None
                    },
                    WsMessage::Frame(_) => unreachable!(),
                }
            },
            // Pass the GStreamer messages to the application control logic
            gst_msg = send_gst_msg_rx.select_next_some() => {
                app.handle_pipeline_message(&gst_msg)?;
                None
            },
            // Handle WebSocket messages we created asynchronously
            // to send them out now
            ws_msg = send_ws_msg_rx.select_next_some() => { Some(ws_msg) },
            // Once we're done, break the loop and return
            else => { break },
        };

        // If there's a message to send out, do so now
        if let Some(ws_msg) = ws_msg {
            ws_sink.send(ws_msg).await?;
        }
    }

    Ok(())
}
