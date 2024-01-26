import socket
import curses
import threading
import logging

logging.basicConfig(filename='client_debug.log', level=logging.DEBUG)

# Client configuration
SERVER_HOST = 'localhost'
SERVER_PORT = 12345

map_data = []
user_messages = []
chat_messages = []

size_y = 16
size_x = 30

COLOR_PAIRS = {
    "GREY": (curses.COLOR_WHITE, curses.COLOR_BLACK),
    "RED": (curses.COLOR_RED, curses.COLOR_BLACK),
    "GREEN": (curses.COLOR_GREEN, curses.COLOR_BLACK),
    "YELLOW": (curses.COLOR_YELLOW, curses.COLOR_BLACK),
    "BLUE": (curses.COLOR_BLUE, curses.COLOR_BLACK),
    "MAGENTA": (curses.COLOR_MAGENTA, curses.COLOR_BLACK),
    "CYAN": (curses.COLOR_CYAN, curses.COLOR_BLACK),
    "PURPLE": (curses.COLOR_MAGENTA, curses.COLOR_BLACK),
    "PINK": (curses.COLOR_RED, curses.COLOR_BLACK),
}

def init_map():
    data = []
    for i in range(size_y):
        row = []
        for j in range(size_x):
            row.append({"char": " ", "color": "GREY"})

        data.append(row)

    return data

def process_data(stdscr, client_socket):
    global map_data, text
    while True:
        data = ""
        while True:
            chunk = client_socket.recv(1024).decode()
            data += chunk

            while ":END" in data:
                message, data = data.split(":END", 1)

                if message.startswith("MAP:"):
                    map_data = init_map()

                    entities = [entity.split(":") for entity in message[4:].split("|")]
                    for entity in entities:
                        map_data[int(entity[4])][int(entity[3])] = {"char": entity[8], "color": entity[5]}
                elif message.startswith("UTEXT:"):
                    user_messages.append(message[6:])
                elif message.startswith("CHAT:"):
                    chat_messages.append(message[5:])

def draw_map(stdscr, client_socket):
    curses.start_color()

    for pair_id, (fg_color, bg_color) in enumerate(COLOR_PAIRS.values(), start=1):
        curses.init_pair(pair_id, fg_color, bg_color)

    chat = False
    message = ""

    while True:
        # Receive the map data from the server

        # Clear the screen
        stdscr.erase()

        # Display the map data on the client side
        for i, row in enumerate(map_data):
            for j, cell in enumerate(row):
                char, color = cell["char"], cell["color"]
                pair_id = list(COLOR_PAIRS.keys()).index(color) + 1
                stdscr.addstr(i, j, char, curses.color_pair(pair_id))

        stdscr.addstr(0, size_x+32, "Chat [Global]")
        for i, j in enumerate(chat_messages[-size_y+1:]):
            stdscr.addstr(i+1, size_x+32, j)

        stdscr.addstr(0, size_x+1, "Info")
        for i, j in enumerate(user_messages[-size_y+1:]):
            stdscr.addstr(i+1, size_x+1, j)

        # Refresh the screen
        #stdscr.refresh()
        curses.doupdate()

        # Check for user input without blocking
        user_input = stdscr.getch()

        if user_input != curses.ERR:
            if not chat and user_input == ord("/"):
                chat = True
            elif chat and user_input == 10:
                client_socket.send(f"CHAT:{message}".encode())
                message = ""
                chat = False
            elif chat and user_input != 10:
                message += chr(user_input)
            else:
                client_socket.send(chr(user_input).encode())

def main(stdscr):
    curses.curs_set(0)  # Hide the cursor
    stdscr.nodelay(1)   # Set non-blocking mode for getch

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as client_socket:
        client_socket.connect((SERVER_HOST, SERVER_PORT))

        # Run the map refresh loop
        process_thread = threading.Thread(target=process_data, args=(stdscr, client_socket))
        process_thread.start()

        draw_map(stdscr, client_socket)

if __name__ == "__main__":
    map_data = init_map()
    curses.wrapper(main)
