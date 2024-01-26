import random
import queue

COLORS = ["GREY", "RED", "GREEN", "YELLOW", "BLUE", "MAGENTA", "CYAN", "PURPLE", "PINK"]

class Entity(object):
    _entities = {}

    def __init__(self, id: str, name: str, type_: str, x: int, y: int, data: str, interact: str, color=random.choice(COLORS)):
        self.id = id
        self.name = name
        self.type_ = type_
        self.x = x
        self.y = y
        self.color = color
        self.data = data
        self.interact = interact

        self.message_queue = queue.Queue()

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

    def enqueue_message(self, message):
        self.message_queue.put(message)

    def __str__(self):
        return f"{self.id}:{self.name}:{self.type_}:{self.x}:{self.y}:{self.color}:{self.data}:{self.interact}:{self.char()}"
