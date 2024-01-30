import socket
import threading
import curses
import logging

logging.basicConfig(filename="log.log", level=logging.DEBUG)

HOST = "localhost"
PORT = 12345

CHUNK_SIZE = 1024

COLOR_PAIRS = {
    "GREY": (curses.COLOR_WHITE, curses.COLOR_BLACK),
    "RED": (curses.COLOR_RED, curses.COLOR_BLACK),
    "GREEN": (curses.COLOR_GREEN, curses.COLOR_BLACK),
    "YELLOW": (curses.COLOR_YELLOW, curses.COLOR_BLACK),
    "BLUE": (curses.COLOR_BLUE, curses.COLOR_BLACK),
    "CYAN": (curses.COLOR_CYAN, curses.COLOR_BLACK),
    "PURPLE": (curses.COLOR_MAGENTA, curses.COLOR_BLACK),
}

height = 0
width = 0

map_data = []
player_data = {
    "room": "",
    "name": "",
    "color": "GREY"
}
console_messages = []
chat_messages = []

def init_map(width, height):
    data = []
    for i in range(height):
        row = []
        for j in range(width):
            row.append({"char": " ", "color": "GREY"})

        data.append(row)

    return data

def process_data(client):
    global map_data, height, width, chat_messages, console_messages
    buffer = ""
    while True:
        chunk = client.recv(CHUNK_SIZE).decode()
        if not chunk:
            break

        buffer += chunk
        while ":END" in buffer:
            message, buffer = buffer.split(":END", 1)
            
            meta, data = message.split(":", 1)

            if meta == "PLAY":
                data = data.split(":") 
                player_data["name"] = data[0]
                player_data["color"] = data[1]
                player_data["x"] = data[2]
                player_data["y"] = data[3]
                player_data["room"] = data[4]

            if meta.startswith("MAP"):
                width, height = [int(x) for x in meta.replace("MAP", "").split(",")]
                map_data = init_map(width, height)
                entities = [entity.split(":") for entity in data.split("|")]
                
                for entity in entities:
                    map_data[int(entity[3])][int(entity[2])] = {
                        "char": entity[5],
                        "color": entity[1]
                    }

            if meta == "CHAT":
                chat_messages.append(data)

            if meta == "CONSOLE":
                console_messages.append(data)

            if meta == "GMSG":
                chat_messages.append(data)
                
def curses_proc(stdscr, client):
    curses.start_color()

    for pair_id, (fg_color, bg_color) in enumerate(COLOR_PAIRS.values(), start=1):
        curses.init_pair(pair_id, fg_color, bg_color)

    # Chat messages
    chat = False
    message = ""

    while True:
        stdscr.erase()

        ## Main Map
        for i, row in enumerate(map_data):
            for j, cell in enumerate(row):
                char, color = cell["char"], cell["color"]
                pair_id = list(COLOR_PAIRS.keys()).index(color) + 1
                stdscr.addstr(i, j, char, curses.color_pair(pair_id))

        ## Console Section
        pair_id = list(COLOR_PAIRS.keys()).index(player_data["color"]) + 1
        stdscr.addstr(0, width+1, "Console [")
        stdscr.addstr(player_data['name'], curses.color_pair(pair_id) | curses.A_BOLD)
        stdscr.addstr("]")
        for i, j in enumerate(console_messages[-height+1:]):
            stdscr.addstr(i+1, width+1, j)

        ## Chat Section
        stdscr.addstr(0, width+48, f"Chat [{player_data['room'].replace('_', ' ').title()}]")
        for i, j in enumerate(chat_messages[-height+1:]):
            try:
                color, name, text = j.split(":")  
                pair_id = list(COLOR_PAIRS.keys()).index(color) + 1
                stdscr.addstr(i+1, width+48, name, curses.color_pair(pair_id) | curses.A_BOLD)
                stdscr.addstr(":" + text)
            except ValueError:
                stdscr.addstr(j)    

        #stdscr.refresh()
        curses.doupdate()

        user_input = stdscr.getch()
        if user_input != curses.ERR:
            if not chat and user_input == ord("/"):
                chat = True
            elif chat and user_input == 10:
                client.send(f"CHAT:{message}".encode())
                message = ""
                chat = False
            elif chat and user_input != 10:
                message += chr(user_input)
            else:
                client.send(("KEY:" + chr(user_input)).encode())

def start_client(stdscr):
    curses.curs_set(0)
    stdscr.nodelay(1)

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as client_socket:
        client_socket.connect((HOST, PORT))

        ## Initialize thread for processing data from server
        process_thread = threading.Thread(target=process_data, args=(client_socket,))
        process_thread.start()

        curses_proc(stdscr, client_socket)

if __name__ == "__main__":
    curses.wrapper(start_client) 
