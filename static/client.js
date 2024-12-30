// Wait for the protobuf.js library to load
window.onload = () => {
    const protoSchema = `
        syntax = "proto3";

        package pixelstrom;

        message ScreenSync {
            uint32 width = 1;
            uint32 height = 2;
            // width * height * 4 bytes (rgba)
            bytes pixels = 3;
        }
    `;
    
    // Parse the schema
    const root = protobuf.parse(protoSchema).root;

    // Get the ScreenSync message type
    const ScreenSync = root.lookupType("ScreenSync");

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
                    // Assuming the data is protobuf-encoded, decode it using your ScreenSync schema
                    const screenSync = ScreenSync.decode(new Uint8Array(buffer));
                    console.log("Decoded ScreenSync:", screenSync);

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

                    // Fill in the ImageData object with pixel data
                    // for (let i = 0; i < screenSync.width * screenSync.height; i++) {
                    //     const idx = i * 4;  // RGBA format (4 bytes per pixel)

                    //     // Fill the ImageData array with RGBA values
                    //     imageData.data[idx] = pixels[i * 4];     // Red
                    //     imageData.data[idx + 1] = pixels[i * 4 + 1]; // Green
                    //     imageData.data[idx + 2] = pixels[i * 4 + 2]; // Blue
                    //     imageData.data[idx + 3] = pixels[i * 4 + 3]; // Alpha
                    // }

                    for (let byte = 0; byte < pixels.length; byte += 4) {
                        imageData.data[byte + 0] = pixels[byte + 0]; // Red
                        imageData.data[byte + 1] = pixels[byte + 1]; // Green
                        imageData.data[byte + 2] = pixels[byte + 2]; // Blue
                        imageData.data[byte + 3] = 255; // Alpha
                    }

                    // Put the ImageData onto the canvas
                    ctx.putImageData(imageData, 0, 0);

                } catch (e) {
                    console.error("Error decoding screenSync:", e);
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
};
