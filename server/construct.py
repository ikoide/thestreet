from models import Room, Entity, Player, Structure, Entrance

STREET_LENGTH = 16

def build_rooms():
    spawn = Room(name="spawn")
    #for i in range(STREET_LENGTH):
    #    room = Room(
    #        name = f"street{i}"
    #    )
    #
    #    for j in range(room.width):
    #        entrance = Entrance(
    #            id = f"entrance_{room.name}_{j},0",
    #            color = "green",
    #            x = j,
    #            y = 0,
    #            room = room,
    #            to_room = f"street{i+1}_{j}{room.height-1}"
    #        )
