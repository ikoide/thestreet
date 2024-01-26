import random
import queue

COLORS = ["GREY", "RED", "GREEN", "YELLOW", "BLUE", "MAGENTA", "CYAN", "PURPLE", "PINK"]

class Entity(object):
    _entities = {}

    def __init__(self, id: str, name: str, type_: str, x: int, y: int, data: str, interact: str, socket=None, color=random.choice(COLORS), room="spawn"):
        self.id = id
        self.name = name
        self.type_ = type_
        self.x = x
        self.y = y
        self.color = color
        self.data = data
        self.interact = interact
        self.room = room

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
    def at_coords(cls, x, y):
        """Returns list of entities currently situated on (x,y) coord."""
        matches = []

        for entity in cls._entities.values():
            if entity.x == x and entity.y == y:
                matches.append(entity)

        return matches[0] if len(matches) > 0 else None

    @classmethod
    def filter_by_attribute(cls, attribute, value):
        return {key: item for key, item in cls._entities.items() if getattr(item, attribute, None) == value}

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

    def __str__(self):
        return f"{self.id}:{self.name}:{self.type_}:{self.x}:{self.y}:{self.color}:{self.data}:{self.interact}:{self.char()}"

class Room(object):
    _rooms = {}

    def __init__(self, name, width=32, height=16):
        self.name = name
        self.width = width
        self.height = height
        self.map_data = self.generate_map()

        Room._rooms[name] = self

    @classmethod
    def find_by_name(cls, name: str):
        return cls._rooms.get(name)
    
    @classmethod
    def get_entities(cls) -> dict:
        """Returns all rooms."""
        return cls._rooms

    def generate_map(self):
        map_data = [[' ' for _ in range(self.width)] for _ in range(self.height)]
    
        for y in range(self.height):
            for x in range(self.width):
                if x == 0 or x == self.width - 1 or y == 0 or y == self.height - 1:
                    entity = Entity(
                        id = f"BORDER{x}{y}",
                        name = "Border",
                        type_ = "border",
                        x = x,
                        y = y,
                        color = "GREY",
                        data = "A border.",
                        interact = "You can't go there.",
                        room = self.name
                    )

        return map_data

    def get_map_data(self):
        return self.map_data
