const protoSchema = `
syntax = "proto3";

package pixelstrom;

// Root message for WebSocket communication
message WebSocketMessage {
  oneof payload {
    ScreenSync screen_sync = 2;
    ClientPainting client_painting = 3;
  }
}

// Entire contents of the screen
message ScreenSync {
    uint32 width = 1;
    uint32 height = 2;
    // width * height * 4 bytes (rgba)
    bytes pixels = 3;
}

// Partial update of the screen after a client finished painting
message ClientPainting {
    // Name of the client that painted the pixels
    string client = 1;

    // List of (2 byte x + 2 byte y + 4 byte (rgba)).
    // Contains multiple entries
    bytes painted = 2;
}
`;

// Parse the schema
const root = protobuf.parse(protoSchema).root;

const ScreenSync = root.lookupType("ScreenSync");
const WebSocketMessage = root.lookupType("WebSocketMessage");

// Wait for the protobuf.js library to load (and other stuff???)
window.onload = () => {
    // Create a WebSocket connection to the server
    const socket = new WebSocket("ws://localhost:3000/ws");

    socket.onmessage = (event) => {
        const arrayBuffer = event.data;
    
        // Check if the data is a Blob (it could be, depending on how the server sends the data)
        if (arrayBuffer instanceof Blob) {
            // Convert the Blob into an ArrayBuffer
            const reader = new FileReader();

            reader.onloadend = () => {
                const buffer = reader.result;
    
                try {
                    const webSocketMessage = WebSocketMessage.decode(new Uint8Array(buffer));
                    applyWebSocketMessage(webSocketMessage);
                } catch (e) {
                    console.error("Error decoding webSocketMessage:", e);
                }
            };
    
            // Read the Blob as an ArrayBuffer
            reader.readAsArrayBuffer(arrayBuffer);
        } else {
            console.error('Expected Blob, but received something else');
        }
    };

    socket.onerror = (error) => {
        console.error('WebSocket Error:', error);
    };

    socket.onopen = () => {
        console.log('WebSocket connection established');
    };

    socket.onclose = () => {
        console.log('WebSocket connection closed');
    };

    fetch("/api/current-screen")
        .then(response => {
            if (!response.ok) {
                throw new Error('Network response was not ok');
            }
            // Read the response body as an ArrayBuffer
            return response.arrayBuffer();
        })
        .then(arrayBuffer => {
            // Convert the ArrayBuffer to a Uint8Array to decode with Protobuf
            const bytes = new Uint8Array(arrayBuffer);
            return ScreenSync.decode(bytes);
        })
        .then(screenSync => {
            // Apply the decoded object
            applyScreenSync(screenSync);
        })
        .catch(error => {
            console.error('Error:', error);
        });

};

function applyScreenSync(screenSync) {
    if (screenSync.width === 0 || screenSync.height === 0) {
        console.error("Invalid screenSync dimensions:", screenSync.width, screenSync.height);
        return;
    }

    // Get pixel data (this assumes pixels are in raw RGBA format)
    const pixels = new Uint8Array(screenSync.pixels);

    // Get the canvas element and its context
    const screen = document.getElementById('screen');
    const ctx = screen.getContext('2d');

    // Set the screen size to the screenSync size
    screen.width = screenSync.width;
    screen.height = screenSync.height;

    // Create ImageData object to hold the pixels
    const imageData = ctx.createImageData(screenSync.width, screenSync.height);

    for (let byte = 0; byte < pixels.length; byte += 4) {
        imageData.data[byte + 0] = pixels[byte + 0]; // Red
        imageData.data[byte + 1] = pixels[byte + 1]; // Green
        imageData.data[byte + 2] = pixels[byte + 2]; // Blue
        imageData.data[byte + 3] = 255; // Alpha
    }

    // Put the ImageData onto the canvas
    ctx.putImageData(imageData, 0, 0);
}

function applyWebSocketMessage(webSocketMessage) {
    console.log("Got WebSocketMessage", webSocketMessage, "with payload", webSocketMessage.payload);
    switch(webSocketMessage.payload) {
        case "screenSync":
            applyScreenSync(webSocketMessage.screenSync);
            break;
    }
}
