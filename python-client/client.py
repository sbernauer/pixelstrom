import socket
import random
import time
from PIL import Image


def load_image(image_path):
    """
    Load the image and prepare a flattened list of pixel data.
    Each pixel is represented as (x, y, color).
    """
    try:
        image = Image.open(image_path)
        image = image.convert("RGB")  # Ensure the image is in RGB mode
        width, height = image.size
        pixels = [(x, y, '{:02x}{:02x}{:02x}'.format(*image.getpixel((x, y))))
                  for y in range(height) for x in range(width)]
        return pixels, width, height
    except Exception as e:
        raise ValueError(f"Failed to load image {image_path}: {e}")


def connect_to_server(host, port, username, password):
    """
    Establish a connection to the server, authenticate, and return the socket.
    """
    client_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    client_socket.connect((host, port))

    # Send login credentials
    client_socket.sendall(f"LOGIN {username} {password}\n".encode('utf-8'))

    # Receive login response and assert success
    login_response = client_socket.recv(1024).decode('utf-8').strip()
    if login_response != "LOGIN SUCCEEDED":
        client_socket.close()
        raise ConnectionError(f"Login failed: {login_response}")
    print("Login succeeded")

    return client_socket


def get_screen_size(client_socket):
    """
    Query the server for the screen size and return the dimensions.
    """
    client_socket.sendall("SIZE\n".encode('utf-8'))
    size_message = client_socket.recv(1024).decode('utf-8').strip()

    if size_message.startswith("SIZE "):
        _, screen_width, screen_height = size_message.split()
        return int(screen_width), int(screen_height)
    else:
        raise ValueError(f"Failed to get screen size: {size_message}")


def calculate_new_position(screen_width, screen_height, image_width, image_height):
    """
    Calculate a new random position for the image, ensuring it fits within screen bounds.
    """
    start_x = random.randint(0, max(screen_width - image_width, 0))
    start_y = random.randint(0, max(screen_height - image_height, 0))
    # print(f"New position for image: ({start_x}, {start_y})")
    return start_x, start_y


def draw_pixels(client_socket, pixels, pixel_count, duration, screen_width, screen_height,
                image_width, image_height, pixel_state):
    """
    Draw pixels from the image within the allowed duration.
    Recalculate a new random position only after the entire image is drawn.
    """
    # If this is the first time drawing, calculate the initial position
    if pixel_state['remaining_image_pixels'] == 0:
        pixel_state['remaining_image_pixels'] = len(pixels)
        pixel_state['start_x'], pixel_state['start_y'] = calculate_new_position(
            screen_width, screen_height, image_width, image_height
        )

    # Start timing for this drawing session
    start_time = time.perf_counter()
    elapsed_time = 0

    # Keep drawing pixels until the batch size is exhausted or time is up
    while pixel_count > 0 and elapsed_time < duration / 1000.0:  # Convert ms to seconds
        # Get the current pixel
        x, y, color = pixels[pixel_state['pixel_index']]
        draw_x = pixel_state['start_x'] + x
        draw_y = pixel_state['start_y'] + y

        # Only send pixels within screen bounds
        if 0 <= draw_x < screen_width and 0 <= draw_y < screen_height:
            client_socket.sendall(f"PX {draw_x} {draw_y} {color}\n".encode('utf-8'))

        # Update counters
        pixel_state['pixel_index'] += 1
        pixel_count -= 1
        pixel_state['remaining_image_pixels'] -= 1

        # Check elapsed time
        elapsed_time = time.perf_counter() - start_time

        # If we finished painting the entire image, reset for the next location
        if pixel_state['remaining_image_pixels'] == 0:
            pixel_state['pixel_index'] = 0
            pixel_state['remaining_image_pixels'] = len(pixels)
            pixel_state['start_x'], pixel_state['start_y'] = calculate_new_position(
                screen_width, screen_height, image_width, image_height
            )

    # Send DONE before the time slot expires
    client_socket.sendall("DONE\n".encode('utf-8'))


def main():
    server_host = '127.0.0.1'
    server_port = 1234
    username = "Sebidooo"
    password = "test123"
    image_path = "cat_sleeping_small.png"

    try:
        # Load the image and get pixel data
        pixels, image_width, image_height = load_image(image_path)

        # Connect to the server and authenticate
        with connect_to_server(server_host, server_port, username, password) as client_socket:
            # Get the screen dimensions
            screen_width, screen_height = get_screen_size(client_socket)
            print(f"Screen size: {screen_width}x{screen_height}")

            # Track pixel drawing state
            pixel_state = {
                'pixel_index': 0,  # Current pixel index to draw
                'remaining_image_pixels': 0,  # Pixels remaining for the current image
                'start_x': 0,  # Current X position of the image
                'start_y': 0   # Current Y position of the image
            }

            while True:
                # Wait for a command from the server
                server_message = client_socket.recv(1024).decode('utf-8').strip()
                if server_message.startswith("START "):
                    # Parse the START command
                    _, pixel_count, duration = server_message.split()
                    pixel_count = int(pixel_count)
                    duration = int(duration)
                    print(f"Received START command: {pixel_count} pixels, {duration}ms")
                    
                    # We meed at least 30ms to flush and stay in time
                    duration_safety_puffer_ms = max(duration / 2, 30)
                    # We need at least 20ms to draw at least *something*
                    duration = max(duration - duration_safety_puffer_ms, 20)
                    print(f"Using duration of {duration}ms, taking the network delay into account. You might need to increase the safety puffer on different networks")

                    # Draw pixels within the given time frame
                    draw_pixels(client_socket, pixels, pixel_count, duration, screen_width,
                                screen_height, image_width, image_height, pixel_state)

                elif server_message.startswith("ERROR "):
                    print(f"[ERROR from server]: {server_message.lstrip('ERROR ')}")
                elif len(server_message) == 0:
                    print("Server closed connection")
                    return
                else:
                    print(f"[DEBUG from server]: {server_message}")

    except KeyboardInterrupt:
        print("Client terminated by user.")
    except Exception as e:
        print(f"An error occurred: {e}")


if __name__ == "__main__":
    main()
