import WebSocket from "ws";

const ws = new WebSocket("ws://localhost:8080/_server");
ws.addEventListener("message", (msg) => {
    let str = JSON.parse(msg.data.toString('utf8'));

    console.log(str);
});

ws.addEventListener("error", console.error);
