import WebSocket, { WebSocketServer } from 'ws';
import { contains } from "cidr-tools";
import { nanoid } from "nanoid";

const wss = new WebSocketServer({ port: 8080, host: '0.0.0.0' });

const localhost = ['::1/128', '127.0.0.0/8', '192.168.128.0/23'];
let current_server: WebSocket | undefined;
let clients: Record<string, WebSocket> = {};

// OpCodes
// 1 = Hello                (Signaling > Client) *
// 2 = Join                 (Client > Signaling)
// 3 = Client Message       (Signaling > Server | Client > Signaling) *
// 4 = Server Message       (Server > Signaling)
// 5 = Client Disconnect    (Signaling > Server)
// 6 = Join ACK             (Signaling > Client) *
// 7 = Join Notification    (Signaling > Server)
// 8 = Server Message       (Signaling > Client | Server > Signaling) *

enum Codes {
    Hello = 'hello',
    Join = 'join',
    ClientMessage = 'client_message',
    ServerMessage = 'server_message',
    ClientDisconnect = 'client_disconnect',
    JoinACK = 'join_ack',
    ClientJoin = 'client_join'
}

wss.on('connection', (ws, request) => {
    ws.on('error', console.error);
    let id: string = nanoid();

    ws.on('message', (data) => {
        try {
            let request = JSON.parse(data.toString('utf8'));

            if (!request) { throw new Error("failed to deserialize"); }

            if (request.type === undefined && request.type === null) {
                throw new Error("invalid packet");
            }

            switch (request.type) {
                case Codes.Join:
                    if (id !== "server" && !clients[id]) {
                        if (current_server) {
                            console.log(":: Client", id, "joined the streaming.");
                            clients[id] = ws;
                            ws.send(JSON.stringify({ type: Codes.JoinACK }));
                            current_server.send(JSON.stringify({ type: Codes.ClientJoin, data: { peer: id } }));
                        } else {
                            console.log(":: Client couldn't connect because sts is not connected");
                        }
                    }
                    break;
                case Codes.ClientMessage:
                    // if we are in the client list.
                    if (clients[id] !== undefined && id !== "server" && current_server) {
                        delete request.type;
                        // In case of a opcode not handled by an opcode.
                        current_server.send(JSON.stringify({
                            type: Codes.ClientMessage, data: {
                                ...request.data,
                                peer: id,
                            }
                        }));
                    }
                case Codes.ServerMessage:
                    if (id === "server") {
                        let receiver = request.data.peer;

                        if (clients[receiver]) {
                            clients[receiver].send(JSON.stringify(request));
                        }
                        console.log(":: Server send", receiver);
                    }
                    break;
                case Codes.ClientDisconnect:
                    if (clients[id] && current_server) {
                        current_server.send(JSON.stringify({ type: Codes.ClientDisconnect, data: { peer: id } }));
                        delete clients[id];
                        console.log(":: Client", id, "left streaming");
                    }
                    break;
                default:
                    throw new Error('invalid ' +  request.type);
                    break;
            }
        } catch (e) {
            console.log(':: Error while handling request', e);
        }
    });

    ws.on('close', () => {
        if (clients[id] && current_server) {
            current_server.send(JSON.stringify({ type: Codes.ClientDisconnect, data: { peer: id } }));
            delete clients[id];
            console.log(":: Client", id, "left streaming");
        } else if (id === 'server') {
            wss.clients.forEach((ws) => ws.close());
            current_server = undefined;
        }
    });

    if (request.url === "/_server" && !current_server /*&& request.socket.remoteAddress && contains(localhost, request.socket.remoteAddress)*/) {
        id = 'server';
        current_server = ws;

        console.log(":: Server logged in");
    }

    if (!current_server) {
        ws.close();
        return;
    } else {
        // send the initialization packet.
        ws.send(JSON.stringify({ type: Codes.Hello, data: { id } }));
    }

});
