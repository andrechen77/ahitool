from PIL import Image
import sys

def ico_to_rgba8(ico_path):
    # Open the .ico file
    img = Image.open(ico_path)

    # Ensure the image is in RGBA format (with alpha channel)
    img = img.convert("RGBA")

    # Get the raw pixel data (as a list of tuples)
    pixels = list(img.getdata())

    # Convert the pixel data to a flat list of RGBA bytes
    rgba8_data = []
    for pixel in pixels:
        rgba8_data.extend(pixel)  # each pixel is a tuple (R, G, B, A)

    # Return the raw RGBA8 data and image size
    return bytes(rgba8_data), img.width, img.height

if __name__ == "__main__":
    # Provide the path to your .ico file
    ico_path = sys.argv[1] if len(sys.argv) > 1 else 'your-icon.ico'

    rgba8_data, width, height = ico_to_rgba8(ico_path)

    # For demonstration, print the result
    print(f"Width: {width}, Height: {height}")
    print(f"Raw RGBA8 data (first 64 bytes): {rgba8_data[:64]}")

    # Optionally, save to a file for debugging or testing
    with open("output.rgba8", "wb") as f:
        f.write(rgba8_data)
