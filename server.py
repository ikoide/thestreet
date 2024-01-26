import socket
import threading
import time
import random
import queue

from entity import Entity

HOST = "localhost"
PORT = 12345

SIZE_X, SIZE_Y = 24, 16
CHUNK_SIZE = 1024
REFRESH_RATE = 0.2
COLORS = ["GREY", "RED", "GREEN", "YELLOW", "BLUE", "MAGENTA", "CYAN", "PURPLE", "PINK"]

def init_player(client):
    random_x = random.randint(1, SIZE_X-2)
    random_y = random.randint(1, SIZE_Y-2)

    player = Entity(
        id = client.getpeername(),
        name = "John",
        type_ = "player",
        x = random_x,
        y = random_y,
        color = random.choice(COLORS),
        data = "This is a player.",
        interact = "Chat",
    )

    return player

def move_player(player, key):
    new_x, new_y = player.x, player.y 

    if key == "w":
        new_y -= 1
    if key == "a":
        new_x -= 1
    if key == "s":
        new_y += 1
    if key == "d":
        new_x += 1

    entity = Entity.at_coords(new_x, new_y)
    if entity:
        player.enqueue_message(entity.interact)
    else:
        player.x, player.y = new_x, new_y

def send_data(client, player):
    try:
        while True:
            if client.fileno() == -1:
                break

            entities = []
            for id, entity in Entity.get_entities().items():
                entities.append(entity.__str__())

            map_data = "MAP:" + "|".join(entities) + ":END"

            try:
                #message = player.message_queue.get(timeout=0.1)
                text_data = "TEXT:" + player.message_queue.get_nowait() + ":END"
            except queue.Empty:
                text_data = ""

            data = map_data + text_data

            ## Breaking data into chunks of CHUNK_SIZE bytes to be sent in segments across TCP stream
            for i in range(0, len(data), CHUNK_SIZE):
                chunk = data[i:i+CHUNK_SIZE]
                print(len(chunk), chunk)
                client.send(chunk.encode())

            time.sleep(REFRESH_RATE)

    except (BrokenPipeError, ConnectionResetError):
        print("Player disconnected.")
    finally:
        client.close()

def receive_data(client, player):
    try:
        while True:
            data = client.recv(1024)

            if not data:
                break

            key = data.decode()
            print(f"Received input from {client.getpeername()}: {key}")

            if key == "q":
                print(f"Player {client.getpeername()} has disconnected.")
                break

            if key in ["w", "a", "s" , "d"]:
                move_player(player, key)
            
    except (BrokenPipeError, ConnectionError):
        print(f"Connection with {client.getpeername()} closed.")
    finally:
        Entity.remove_entity(client.getpeername())



def handle_player(client):
    print(f"Accepted connection from {client.getpeername()}")

    player = init_player(client)

    data_sender = threading.Thread(target=send_data, args=(client,player))
    data_sender.start()

    data_receiver = threading.Thread(target=receive_data, args=(client,player))
    data_receiver.start()

def init_entities():
    """Generates border entities around the map."""
    for y in range(SIZE_Y):
        for x in range(SIZE_X):
            if x == 0 or x == SIZE_X - 1 or y == 0 or y == SIZE_Y - 1:
                entity = Entity(
                    id = f"BORDER{x}{y}",
                    name = "Border",
                    type_ = "border",
                    x = x,
                    y = y,
                    color = "GREY",
                    data = "A border.",
                    interact = "You can't go there ;(",
                )

def start_server():
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    ## Allows us to use the same IP & Port without having to kill running Python instances manually
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind((HOST, PORT))
    server.listen()
    print(f"Server listening on {HOST}:{PORT}")

    init_entities()

    while True:
        client, address = server.accept()

        player_handler = threading.Thread(target=handle_player, args=(client,))
        player_handler.start()

if __name__ == "__main__":
    start_server()
