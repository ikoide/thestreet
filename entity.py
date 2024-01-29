import random
import queue

COLORS = ["GREY", "RED", "GREEN", "YELLOW", "BLUE", "MAGENTA", "CYAN", "PURPLE", "PINK"]

class Entity(object):
    _entities = {}

    def __init__(self, id: str, name: str, type_: str, x: int, y: int, data: str, interact: str, room, socket=None, color=random.choice(COLORS), dest_room=None):
        self.id = id
        self.name = name
        self.type_ = type_
        self.x = x
        self.y = y
        self.color = color
        self.data = data
        self.interact = interact
        self.room = room
        self.dest_room = dest_room

        self.message_queue = queue.Queue()
        self.socket = socket

        ## Add new Entity object to _entities Class list
        Entity._entities[id] = self

    @classmethod
    def find_by_id(cls, id: str):
        return cls._entities.get(id)
    
    @classmethod
    def get_entities(cls) -> dict:
        """Returns all entities."""
        return cls._entities

    @classmethod
    def at_coords(cls, room, x, y):
        """Returns list of entities currently situated on (x,y) coord."""
        matches = []

        for entity in cls._entities.values():
            if entity.x == x and entity.y == y and entity.room == room:
                matches.append(entity)

        return matches[0] if len(matches) > 0 else None

    @classmethod
    def in_room(cls, room_name):
        return [entity for entity in cls._entities.values() if entity.room == room_name]

    @classmethod
    def by_type(cls, type_: str) -> list:
        return [entity for entity in cls._entities.values() if entity.type_ == type_]

    @classmethod
    def remove_entity(cls, id: str) -> bool:
        """Class method to remove an entity by its ID."""
        if id in cls._entities:
            del cls._entities[id]
            return True

        return False

    def char(self):
        """Returns character representation of entity type"""
        if self.type_ == "player":
            return "P"
        if self.type_ == "daemon":
            return "D"
        if self.type_ == "border":
            return "#"
        if self.type_ == "entrance":
            return "E"

    def enqueue_message(self, message):
        self.message_queue.put(message)

    def to_room(self, room_name, player, coords):
        room = Room.find_by_name(room_name)
        if room.whitelist and player.name not in room.whitelist:
            return "Fuck you."
        else:
            player.room = room.name
            player.x = coords[0]
            player.y = coords[1]

            return f"You have entered {room.name}."

    def __str__(self):
        return f"{self.id}:{self.name}:{self.type_}:{self.x}:{self.y}:{self.color}:{self.data}:{self.interact}:{self.char()}:{self.room}"

class Room(object):
    _rooms = {}

    def __init__(self, name, width, height, spawn_x=10, spawn_y=10, whitelist=None):
        self.name = name
        self.width = width
        self.height = height
        self.spawn_x = spawn_x
        self.spawn_y = spawn_y
        self.whitelist = whitelist

        self.generate_map()

        Room._rooms[name] = self

    @classmethod
    def find_by_name(cls, name: str):
        return cls._rooms.get(name)
    
    @classmethod
    def get_entities(cls) -> dict:
        """Returns all rooms."""
        return cls._rooms

    def update_entity_at_pos(self, room, x, y):
        entity = Entity.at_coords(room, x, y)
        Entity.remove_entity(entity.id)

    def generate_map(self):
        for y in range(self.height):
            for x in range(self.width):
                if x == 0 or x == self.width - 1 or y == 0 or y == self.height - 1:
                    entity = Entity(
                        id = f"BORDER{self.name}{x}{y}",
                        name = "Border",
                        type_ = "border",
                        x = x,
                        y = y,
                        color = "GREY",
                        data = "A border.",
                        interact = "You can't go there.",
                        room = self.name
                    )

        return None
