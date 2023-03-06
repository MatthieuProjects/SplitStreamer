import { EventEmitter } from "events";

export enum Status {
    Connecting = 'Connecting',
    Disconnected = "Disconnected",
    Registering = "Registering",

    CreatingWebRTC = 'Creating WebRTC',

    Transmitting = 'Transmitting',

    Ready = "Ready",
}

enum Codes {
    Hello = 'hello',
    Join = 'join',
    ClientMessage = 'client_message',
    ServerMessage = 'server_message',
    ClientDisconnect = 'client_disconnect',
    JoinACK = 'join_ack',
    ClientJoin = 'client_join'
}

type Payload = { type: Codes, data: any };

const rtcConfig: RTCConfiguration = {
    iceServers: [{ urls: "stun:stun.services.mozilla.com" },
    { urls: "stun:stun.l.google.com:19302" }]
};

export default class WebRtcManager extends EventEmitter {
    #websocket?: WebSocket;
    #connection?: RTCPeerConnection;
    #stream?: MediaStream;
    #istatus: Status = Status.Disconnected;
    #retrys: number = 0;

    set #status(value: Status) {
        this.#istatus = value;
        this.emit('status', value);
    }

    get status() {
        return this.#istatus;
    }

    transmit() {
        if (this.#connection === undefined && this.#websocket !== undefined) {
            this.#websocket?.send(JSON.stringify({ type: Codes.Join }));
        } else {
            throw new Error("Inconsistent state, can't start a stream in this state");
        }
    }

    stopTransmit() {
        if (this.#connection && this.#websocket) {
            this.#websocket.send(JSON.stringify({ type: Codes.ClientDisconnect }));
        } else {
            throw new Error("Inconsistent state, can't stop a stream in this state");
        }
    }

    constructor() {
        super();
        this.#start();
    }

    async #start() {
        this.#status = Status.Connecting;

        // Before starting, we need to go back to an initial state.
        if (this.#connection) {
            this.#connection.close();
            this.#connection = undefined;
        }

        // Stop any started stream.
        if (this.#stream) {
            this.#stream.getTracks().forEach(function (track) {
                track.stop();
            });
        }

        // we need a new session.
        if (this.#websocket && this.#websocket.readyState === this.#websocket.OPEN) {
            this.#websocket!.send(JSON.stringify({ type: Codes.ClientDisconnect }));
            this.#websocket.close();
        }

        await new Promise((r) => setTimeout(() => r(void 0), this.#retrys * 1000));
        this.#retrys++;

        const ws = `ws://localhost:8080/`;
        console.log(`Connecting to ${ws}`);
        this.#websocket = new WebSocket(ws);

        this.#websocket.addEventListener('open', () => {
            console.log(':: WebSocket connection openned');
            this.#status = Status.Registering;
        });

        this.#websocket.addEventListener('error', (error) => {
            console.log(':: WebSocket connection error: ', error);

            // In case of an error, we restart everything.
            this.#start();
        });

        this.#websocket.addEventListener('close', () => {
            // If our status is connecting, we shouldn't reset and start a new connection.
            if (this.status !== Status.Connecting) {
                return;
            }

            this.#status = Status.Disconnected;

            // Our status will be reset by the start method.
            window.setTimeout(this.#start.bind(this), 0);
        });

        this.#websocket.addEventListener('message', async (event) => {
            let payload: Payload;
            // We try to deserialize the payload
            try {
                payload = JSON.parse(event.data);
            } catch (e) {
                console.log(':: Failed to deserialize payload');
                // We reset the connection.
                window.setTimeout(this.#start.bind(this), 0);
                return;
            }

            switch (payload.type) {
                // Our connection is now initialized and we need to wait until our client wants to start streaming.
                case Codes.Hello:
                    console.log(':: Connection initialized');
                    this.#status = Status.Ready;
                    break;
                // Once out client has decided to start streaming, we need to wait for the signaling server to tell us
                // that we can start streaming.
                case Codes.JoinACK:
                    console.log(':: Creating RTCPeerConnection');
                    this.#connection = new RTCPeerConnection(rtcConfig);

                    // When our client has found an ice candidate, we forward it to the server.
                    this.#connection.addEventListener('icecandidate', (event) => {
                        // We have a candidate, send it to the remote party with the
                        // same uuid
                        if (event.candidate == null) {
                            console.log(":: Found a null ICE candidate");
                            return;
                        }
                        console.log(':: Sending ICE candidate');
                        this.#websocket!.send(JSON.stringify({ type: Codes.ClientMessage, data: { type: 'ice', 'data': event.candidate } }));
                    });

                    /* Send our video/audio to the other peer */
                    let stream: MediaStream;

                    try {
                        stream = await navigator.mediaDevices.getDisplayMedia({ video: true, audio: true });
                    } catch (e) {
                        console.log(':: User declined the media stream request, disconnecting from room.');
                        window.setTimeout(this.#start.bind(this), 0);
                        return;
                    }

                    console.log(':: Handled local stream');
                    for (let track of stream?.getTracks()!) {
                        // If a stream is finished, we need to stop transmitting.
                        track.addEventListener('ended', () => {
                            if (this.status === Status.Transmitting) {
                                window.setTimeout(this.#start.bind(this), 0);
                            }
                        })
                        console.log(':: Adding stream', track);
                        this.#connection!.addTrack(track, stream!);
                    }
                    // Saving the stream.
                    this.#stream = stream;
                    this.emit('video', stream);

                    try {
                        // Creating an offer to send to the server.
                        let offer = await this.#connection.createOffer();
                        console.log(':: Got local description', offer);
                        await this.#connection!.setLocalDescription(offer);
                        this.#status = Status.CreatingWebRTC;
                        this.#websocket!.send(JSON.stringify({ type: Codes.ClientMessage, data: { type: 'sdp', data: this.#connection!.localDescription } }));
                    } catch (e) {
                        window.setTimeout(this.#start.bind(this), 0);
                    }
                    break;
                case Codes.ServerMessage:
                    if (this.#connection) {
                        let data = payload.data;

                        // Handle incoming JSON SDP and ICE messages
                        if (data.type === 'ice') {
                            console.log(':: Got ICE from server');
                            var candidate = new RTCIceCandidate(data.data);
                            try {
                                await this.#connection!.addIceCandidate(candidate);
                            } catch (e) {
                                console.log(':: Failed to add ICE candidate', e);
                                window.setTimeout(this.#start.bind(this), 0);
                            }
                        } else if (data.type === 'sdp') {
                            console.log(':: Got SDP from server');
                            try {
                                await this.#connection!.setRemoteDescription(data.data);
                            } catch(e) {
                                console.log(':: Failed to add SDP candidate', e);
                                window.setTimeout(this.#start.bind(this), 0);
                            }
                            this.#status = Status.Transmitting;
                        } else {
                            console.log(':: Invalid server payload.');
                            window.setTimeout(this.#start.bind(this), 0);
                        }
                    }
                    break;
            }
        });
    }
}