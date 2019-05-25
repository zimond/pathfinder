OBJ_DIR=build
TARGET_DIR=build
SRC_DIR=.

CFLAGS=-Wall -g -O0 -I../../c/include
LIBS=-lpathfinder_c
LDFLAGS=-L../../target/debug
MKDIR=mkdir -p
RM=rm

UNAME=$(shell uname -s)
ifeq ($(UNAME),Darwin)
	# FIXME(pcwalton): Don't link against HarfBuzz!!
	LIBS+=-framework OpenGL -framework CoreFoundation -framework CoreGraphics -framework CoreText
	LIBS+=-lharfbuzz
else
	LIBS+=-lGL
endif

all:	$(TARGET_DIR)/c_canvas_minimal

.PHONY:	clean

$(TARGET_DIR)/c_canvas_minimal:	$(OBJ_DIR)/c_canvas_minimal.o
	$(MKDIR) $(TARGET_DIR) && $(CC) $(LDFLAGS) $(LIBS) `sdl2-config --libs` -o $@ $<

$(OBJ_DIR)/%.o:	$(SRC_DIR)/%.c
	$(MKDIR) $(OBJ_DIR) && $(CC) -c $(CFLAGS) `sdl2-config --cflags` -o $@ $<

clean:
	$(RM) -rf $(TARGET_DIR) && $(RM) -rf $(OBJ_DIR)
