import queue

class Room(object):
    _rooms = {}

    def __init__(self, name, width=32, height=16, whitelist=None):
        self.name = name
        self.width = width
        self.height = height
        self.whitelist = whitelist

        self.entities = {}

        self.generate_map()
        Room._rooms[name] = self

    @classmethod
    def find_by_name(cls, name: str):
        return cls._rooms.get(name)

    def get_entities(self, entity_type="any"):
        entity_class = globals().get(entity_type.title())
        if entity_class and issubclass(entity_class, Entity):
            return [entity for entity in self.entities.values() if isinstance(entity, entity_class)]
        else:
            return self.entities.values()

    def remove_entity(self, id):
        if id in self.entities:
            del self.entities[id]
            return True

        return False

    def at_coords(self, x, y):
        matches = []

        for entity in self.entities.values():
            if entity.x == x and entity.y == y:
                matches.append(entity)

        return matches

    @classmethod
    def is_player(cls, player_name):
        for room in cls._rooms.values():
            for entity in room.entities.values():
                if isinstance(entity, Player) and entity.id == player_name:
                    return room, entity

        return False 

    def get_players(self):
        return [entity for entity in self.entities.values() if isinstance(entity, Player)]

    def generate_map(self):
        for y in range(self.height):
            for x in range(self.width):
                if x == 0 or x == self.width - 1 or y == 0 or y == self.height - 1:
                    border = Structure(
                        id = f"border_{self.name}_{x},{y}",
                        color = "GREY",
                        x = x,
                        y = y,
                        room = self,
                        char = "#"
                    )

class Entity(object):
    def __init__(self, id: str, color: str, x: int, y: int, room: Room, char: str):
        self.id = id
        self.color = color
        self.x = x 
        self.y = y
        self.room = room
        self.char = char

        entity_query = self.room.at_coords(x, y)
        if len(entity_query) > 0:
            self.room.remove_entity(entity_query[0].id)

        self.room.entities[id] = self

    def get_type(self):
        return self.__class__.__name__.lower() 

    def __str__(self):
        return f"{self.id}:{self.color}:{self.x}:{self.y}:{self.room.name}:{self.char}" 

class Structure(Entity):
    def __init__(self, id: str, color: str, x: int, y: int, room: Room, char: str):
        super().__init__(id, color, x, y, room, char)

class Entrance(Structure):
    def __init__(self, id: str, color: str, x: int, y: int, room: Room, to_room: str, char: str):
        super().__init__(id, color, x, y, room, char)

        self.to_room = to_room

class Player(Entity):
    def __init__(self, id: str, color: str, x: int, y: int, room: Room, char: str, socket):
        super().__init__(id, color, x, y, room, char)

        self.message_queue = queue.Queue()
        self.socket = socket

    def to_room(self, room_name, x, y):
        room = Room.find_by_name(room_name)
        self.room.remove_entity(self.id)
        self.room = room
        room.entities[self.id] = self

        self.x = int(x)
        self.y = int(y)

        return f"You have entered {room_name}."

    def move(self, key):
        new_x, new_y = self.x, self.y

        if key == "w":
            new_y -= 1
        if key == "a":
            new_x -= 1
        if key == "s":
            new_y += 1
        if key == "d":
            new_x += 1

        entity_query = self.room.at_coords(new_x, new_y)
        if len(entity_query) == 1:
            entity = entity_query[0]
            if entity.get_type() == "structure":
                self.message_queue.put("You can't go there.")
            if entity.get_type() == "entrance":
                room_name, x, y = entity.to_room.split("_")
                self.message_queue.put(self.to_room(room_name, x, y))

        if len(entity_query) == 0:
            self.x, self.y = new_x, new_y
