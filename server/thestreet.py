import socket
import threading
import random
import time
import queue

from models import Room, Entity, Player
from construct import build_rooms

HOST = "localhost"
PORT = 12345

CHUNK_SIZE = 1024
REFRESH_RATE = 0.1
PROXIMITY_DISTANCE = 8

COLORS = ["GREY", "RED", "GREEN", "YELLOW", "BLUE", "CYAN", "PURPLE"]

def send_chunks(data, player):
    for i in range(0, len(data), CHUNK_SIZE):
        chunk = data[i:i+CHUNK_SIZE]
        player.socket.send(chunk.encode())

def send_data(player):
    try:
        while True:
            if player.socket.fileno() == -1:
                break

            entities = [entity.__str__() for entity in player.room.entities.values()]
            
            map_data = f"MAP{player.room.width},{player.room.height}:" + "|".join(entities) + ":END"
            player_data = "PLAY:" + player.__str__() + ":END"

            try:
                console_data = "CONSOLE:" + player.message_queue.get_nowait() + ":END"
            except queue.Empty:
                console_data = ""

            send_chunks(player_data, player)
            send_chunks(map_data, player)
            send_chunks(console_data, player)

            time.sleep(REFRESH_RATE)

    except (BrokenPipeError, ConnectionRefusedError):
        broadcast_message(f"{player.id} has disconnected.", room=player.room) 
    finally:
        player.socket.close()

def broadcast_chat(player, message):
    players = player.room.get_entities(entity_type="player")
    for player_ in players:
        distance = abs(player.x - player_.x) + abs(player.y - player_.y)
        if distance <= PROXIMITY_DISTANCE:
            try:
                player_.socket.send(("CHAT:" + player.color + ":" + player.id + ": " + message + ":END").encode())
            except socket.error:
                pass

def broadcast_message(message, room=None):
    if room:
        pass
    else:
        players = room.get_entities(entity_type="player")
        for player in players:
            try:
                player.socket.send(("GMSG:" + message + ":END").encode())
            except socket.error:
                pass
     

def receive_data(player):
    try:
        while True:
            message = player.socket.recv(1024).decode()
            if not message:
                break

            meta, data = message.split(":", 1)

            if meta == "KEY":
                if data in ["w", "a", "s", "d"]:
                    player.move(data)
                if data == "q":
                    break

            if meta == "CHAT":
                broadcast_chat(player, data)

    except (BrokenPipeError, ConnectionError):
        pass
    finally:
        broadcast_message(f"{player.id} has disconnected.", room=player.room) 
        player.room.remove_entity(player.id)


def handle_player(client):
    handle = f"John{random.randint(0,100)}"
    room = Room.find_by_name("spawn")

    player = Player(
        id = handle,
        color = random.choice(COLORS),
        x = random.randint(1, room.width-2),
        y = random.randint(1, room.height-2),
        room = room,
        char = "P",
        socket = client
    ) 

    data_sender_thread = threading.Thread(target=send_data, args=(player,))
    data_sender_thread.start()

    data_receiver_thread = threading.Thread(target=receive_data, args=(player,))
    data_receiver_thread.start()

def start_server():
    print("Starting The Street...") 

    ## Build rooms
    build_rooms()

    server_socket = socket.socket()

    ## Allows re-use of the same IP:PORT without killing previous instance manually
    server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server_socket.bind((HOST, PORT))
    server_socket.listen()
    print(f"The Street: Listening for connections on {HOST}:{PORT}")

    while True:
        client, address = server_socket.accept()

        player_handler = threading.Thread(target=handle_player, args=(client,))
        player_handler.start()

if __name__ == "__main__":
    start_server() 
