import socket
import threading
import time
import random
import queue

# Server configuration
HOST = 'localhost'
PORT = 12345

size_y = 16
size_x = 32
refresh_rate = 0.09
chunk_size = 1024

lock = threading.Lock()
elock = threading.Lock()

colors = ["GREY", "RED", "GREEN", "YELLOW", "BLUE", "MAGENTA", "CYAN", "PURPLE", "PINK"]

class Entity(object):
    _entities = {}

    def __init__(self, id: str, name: str, type_: str, x: int, y: int, data: str, interact: str, color=random.choice(colors)):
        self.id = id
        self.name = name
        self.type_ = type_
        self.x = x
        self.y = y
        self.color = color
        self.data = data
        self.interact = interact

        self.message_queue = queue.Queue()

        Entity._entities[id] = self

    @classmethod
    def find_by_id(cls, id: str):
        return cls._entities.get(id)

    @classmethod
    def get_entities(cls) -> dict:
        """Class method to get all entities."""
        return cls._entities

    @classmethod
    def at_coords(cls, x, y):
        matches = []

        for entity in cls._entities.values():
            if entity.x == x and entity.y == y:
                matches.append(entity)

        return matches

    @classmethod
    def remove_entity(cls, id: str) -> bool:
        """Class method to remove an entity by its ID."""
        if id in cls._entities:
            del cls._entities[id]
            return True
        return False

    def char(self):
        if self.type_ == "player":
            return "P"
        if self.type_ == "border":
            return "#"

    def enqueue_text_message(self, message):
        self.message_queue.put(message)

    def __str__(self):
        return f"{self.id}:{self.name}:{self.type_}:{self.x}:{self.y}:{self.color}:{self.data}:{self.interact}:{self.char()}"

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

    entities = Entity.at_coords(new_x, new_y)
    if len(entities) > 0:
        entity = entities[0]
        if entity.type_ == "border":
            player.enqueue_text_message("You can not move there :(")
        if entity.type_ == "player":
            player.enqueue_text_message(f"It's {entity.name}!")
    else:
        player.y, player.x = new_y, new_x

def initialize_player(client_socket, player_id):
    random_x = random.randint(1, size_x-2)
    random_y = random.randint(1, size_y-2)

    player = Entity(
        id = player_id,
        name = "John",
        type_ = "player",
        x = random_x,
        y = random_y,
        color = random.choice(colors),
        data = "This is another player.",
        interact = "Chat",
    )

    return player

def send_map_data(client_socket, entity_id):
    try:
        while True:
            # Send the map data to the player
            if client_socket.fileno() == -1:
                break

            entities = []

            for id, entity in Entity.get_entities().items():
                entities.append(entity.__str__())

            entities = "MAP:" + "|".join(entities) + ":END"
            for i in range(0, len(entities), chunk_size):
                chunk = entities[i:i+chunk_size]
                print(len(chunk))
                client_socket.send(chunk.encode())

            entity = Entity.find_by_id(entity_id)
            try:
                text_message = entity.message_queue.get(timeout=0.1)
                client_socket.send(f"TEXT:{text_message}:END".encode())
            except queue.Empty:
                pass

            time.sleep(refresh_rate)  # Adjust the update frequency

    except (BrokenPipeError, ConnectionResetError):
        print("Player disconnected.")
    finally:
        # Close the socket when the loop exits
        client_socket.close()

def handle_player(client_socket, address):
    print(f"Accepted connection from {address}")

    player = initialize_player(client_socket, address)

    # Create a thread to constantly send map data to the player
    map_sender_thread = threading.Thread(target=send_map_data, args=(client_socket,player.id))
    map_sender_thread.start()

    try:
        while True:
            # Receive player data (if needed)
            data = client_socket.recv(1024)

            # Process player data (if needed)
            if not data:
                break

            user_input = data.decode()
            print(f"Received input from {address}: {user_input}")
            if user_input == "q":
                print(f"Player {address} has disconnected.")
                break

            if user_input in ["w", "a", "s", "d"]:
                move_player(player, user_input)
                

    except (BrokenPipeError, ConnectionResetError):
        print(f"Connection with {address} closed.")
    finally:
        # Close the socket when the loop exits
        
        #set_map_data(player_data["x"], player_data["y"], "#")
        Entity.remove_entity(address)

        client_socket.close()

def start_server():
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind((HOST, PORT))
    server.listen()

    print(f"Server listening on {HOST}:{PORT}")

    ## Init map
    for y in range(size_y):
        for x in range(size_x):
            if y == 0 or x == 0 or x == size_x - 1 or y == size_y - 1:
                entity = Entity(
                    id = f"BORDER{x}{y}",
                    name = "Border",
                    type_ = "border",
                    x = x,
                    y = y,
                    color = "GREY",
                    data = "A border.",
                    interact = "a",
                ) 


    while True:
        client_socket, address = server.accept()
        
        # Create a new thread to handle the player
        player_handler = threading.Thread(target=handle_player, args=(client_socket, address))
        player_handler.start()

if __name__ == "__main__":
    start_server()
