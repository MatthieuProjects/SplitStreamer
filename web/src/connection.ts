import { startCapture } from "./input";

enum Status {
    Disconnected = "Disconnected",
    Registered = "Registered",
    SDPReceived = "SDP Received",
    SDPOffered = "SDP Offered",
    CreatingAnswer = "Creatng Answer",
    SendingSDP = "Sending SDP",
    Registering = "Registering",
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

export default class WebRtcManager {
    #websocket?: WebSocket;
    #attempts: number = 0;
    #connection?: RTCPeerConnection;
    #stream_promise?: Promise<unknown>;

    start() {
        if (this.#connection === undefined) {
            this.#websocket?.send(JSON.stringify({ type: Codes.Join }));
        }
    }

    #status: Status = Status.Disconnected;

    #self_id?: string;
    /**
     * Starts the connection to the websocket signaling server.
     * @returns 
     */
    connect() {
        this.#attempts++;

        // Avoid infinite attempts
        // todo: Replace with exonential backoff
        if (this.#attempts > 3) {
            console.log("Backing off...");
            return;
        }

        const ws = `ws://localhost:8080/`;

        console.log(`Connecting as ${this.#self_id} to ${ws}`);

        // Connect to the websocket server.
        this.#websocket = new WebSocket(ws);

        this.#websocket.addEventListener('open', this.#wsOpen.bind(this));
        this.#websocket.addEventListener('error', this.#wsError.bind(this));
        this.#websocket.addEventListener('message', this.#wsMessage.bind(this));
        this.#websocket.addEventListener('close', this.#wsClose.bind(this));
    }
    /**
     * Resets the state of the connector.
     */
    #resetState() {
        this.#websocket?.close();
    }

    #emitStatus(status: Status) {
        console.log("STATUS CHANGE: ", this.#status, "=>", status);
        this.#status = status;
    }

    #handleError(error: unknown) {
        console.log("ERROR HANDLED: ", error);
        this.#resetState();
    }

    async #createCall(): Promise<unknown> {
        // Reset connection attempts because we connected successfully
        this.#attempts = 0;
        console.log('Creating RTCPeerConnection');

        this.#connection = new RTCPeerConnection(rtcConfig);

        /* Send our video/audio to the other peer */
        this.#stream_promise = startCapture({ video: true, audio: true }).then((stream) => {
            console.log('Adding local stream s = ', stream?.getTracks());
            for (let track of stream?.getTracks()!) {
                this.#connection!.addTrack(track, stream!);
            }
            return stream;
        }).catch(this.#handleError.bind(this));

        this.#connection.onicecandidate = (event) => {
            // We have a candidate, send it to the remote party with the
            // same uuid
            if (event.candidate == null) {
                console.log("ICE Candidate was null, done");
                return;
            }
            this.#websocket!.send(JSON.stringify({ type: Codes.ClientMessage, data: { type: 'ice', 'data': event.candidate } }));
        };

        return this.#stream_promise;
    }

    #incomingSDP(sdp: RTCSessionDescriptionInit) {
        console.log(sdp);
        this.#connection!.setRemoteDescription(sdp).then(() => {
            this.#emitStatus(Status.SDPReceived);

            if (sdp.type != "offer")
                return;
            this.#emitStatus(Status.SDPOffered);

            this.#stream_promise!.then(() => {
                this.#emitStatus(Status.CreatingAnswer);
                this.#connection!.createAnswer()
                    .then(this.#onLocalDescription.bind(this)).catch(this.#handleError.bind(this));
            }).catch(this.#handleError.bind(this));
        }).catch(this.#handleError.bind(this));

    }

    #incomingICE(ice: RTCIceCandidateInit) {
        console.log(ice);
        var candidate = new RTCIceCandidate(ice);
        this.#connection!.addIceCandidate(candidate).catch(this.#handleError.bind(this));
    }

    #onLocalDescription(description: RTCSessionDescriptionInit) {
        console.log("Got local description: " + JSON.stringify(description));
        this.#connection!.setLocalDescription(description).then(() => {
            this.#emitStatus(Status.SendingSDP);
            let sdp = { 'sdp': this.#connection!.localDescription }
            this.#websocket!.send(JSON.stringify({ type: Codes.ClientMessage, data: { type: 'sdp', data: sdp.sdp } }));
        });

    }

    #generateOffer() {
        this.#connection?.createOffer().then(this.#onLocalDescription.bind(this)).catch(this.#handleError.bind(this));
    }

    #wsOpen() {
        this.#emitStatus(Status.Registering);
    }
    #wsError() {
        this.#handleError("Unable to connect to server, did you add an exception for the certificate?")
    }

    #wsMessage(event: MessageEvent) {
        try {
            let message: Payload = JSON.parse(event.data);

            switch (message.type) {
                case Codes.Hello:
                    this.#self_id = message.data.id;
                    this.#emitStatus(Status.Registered);
                    break;
                case Codes.JoinACK:
                    // to handle!
                    this.#createCall().then(this.#generateOffer.bind(this));
                    break;
                case Codes.ServerMessage:
                    if (this.#connection) {
                        // to handle!
                        let data = message.data;

                        // Handle incoming JSON SDP and ICE messages
                        try {
                            if (data.type === 'ice') {
                                this.#incomingICE(data.data);
                            } else if (data.type === 'sdp') {
                                this.#incomingSDP(data.data);
                            } else {
                                this.#handleError("Unknown incoming JSON: " + message);
                            }
                        } catch (e) {
                            if (e instanceof SyntaxError) {
                                this.#handleError("Error parsing incoming JSON: " + event.data);
                            } else {
                                this.#handleError("Unknown error parsing response: " + event.data);
                            }
                            return;
                        }
                    }
                    break;
            }
        } catch (e) {
            this.#handleError(e);
        }
    }
    #wsClose() {
        this.#emitStatus(Status.Disconnected);

        if (this.#connection) {
            this.#connection.close();
            this.#connection = undefined;
        }

        // Reset after a second
        window.setTimeout(this.connect.bind(this), 1000);
    }

}