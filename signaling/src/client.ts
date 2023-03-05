import WebSocket from "ws";

const ws = new WebSocket("ws://localhost:8080");
ws.addEventListener("message", (msg) => {
    let str = JSON.parse(msg.data.toString('utf8'));

    console.log(str);

    if (str.type === 1) {
        // we want to join the streaming.
        ws.send(JSON.stringify({ type: 2 }));
    }
    if (str.type === 6) {
        // we are now connected!
        ws.send(JSON.stringify({ type: 3, op: 1, sdp: "super sdp" }));
    }
});

ws.addEventListener("error", console.error);
