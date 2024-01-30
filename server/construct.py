from models import Room, Entity, Player, Structure, Entrance

STREET_LENGTH = 32

def build_rooms():
    spawn = Room(name="spawn")
    for i in range(STREET_LENGTH):
        room = Room(
            name = f"street{i}",
            height = 16
        )
    
        for j in range(1, room.width-1):
            if i != STREET_LENGTH - 1:
                Entrance(
                    id = f"entrance_{room.name}_{j},0",
                    color = "GREEN",
                    x = j,
                    y = 0,
                    room = room,
                    to_room = f"street{i+1}_{j}_{room.height-2}",
                    char = "E"
                )

            if i != 0:
                Entrance(
                    id = f"entrance_{room.name}_{j},{room.height-1}",
                    color = "GREEN",
                    x = j,
                    y = room.height-1,
                    room = room,
                    to_room = f"street{i-1}_{j}_1",
                    char = "E"
                )
