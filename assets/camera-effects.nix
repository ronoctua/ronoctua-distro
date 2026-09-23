# The camera's effects: what exists, what each one takes, and the numbers.
#
# ONE DECLARATION, and three things come out of it (`nix/camera-effects.nix`):
# the Rust table `ronoctua-effects` includes, the text keys both languages owe
# (`camera-effect-<id>` and `camera-param-<name>`, checked by
# `camera-effects-have-text`), and the refusal of a declaration that cannot be
# drawn — a default outside its own range, a step of zero, two effects with
# one id.
#
# THE ORDER IS THE RING. `e` in the camera app walks it forwards, `E` back, and
# the settings view lists it as it is written here.
#
# Every parameter is an INTEGER. A person turns them with a key or a wheel, one
# step at a time, and a step that is a whole number is one that means the same
# on the screen, in the file and in the log.
#
# `mask = true` is an effect that needs to know where the PERSON is — the
# segmentation model's answer (`ronoctua-segment`). The studio runs the model
# only while one of those is in a chain, which is what keeps a camera with no
# background effect from paying 25 ms a mask.
#
# COLOUR IN AN EFFECT IS ARITHMETIC, NOT A TOKEN. Invariant 9 is about the
# interface — what the system draws around content. An effect transforms the
# CONTENT, and a sepia matrix or a thermal ramp is part of what the effect IS,
# the way a blur radius is. The thermal ramp is computed (a hue sweep), never a
# list of hex values; see `docs/ARCHITECTURE.md`.
{
  effects = [
    {
      id = "grey";
      family = "colour";
      params = [
        {
          name = "amount";
          min = 0;
          max = 100;
          default = 100;
          step = 5;
        }
      ];
    }
    {
      id = "sepia";
      family = "colour";
      params = [
        {
          name = "amount";
          min = 0;
          max = 100;
          default = 100;
          step = 5;
        }
      ];
    }
    {
      id = "negative";
      family = "colour";
      params = [ ];
    }
    {
      id = "posterize";
      family = "colour";
      params = [
        {
          name = "levels";
          min = 2;
          max = 16;
          default = 4;
          step = 1;
        }
      ];
    }
    {
      id = "thermal";
      family = "colour";
      params = [ ];
    }
    {
      id = "night";
      family = "colour";
      params = [
        {
          name = "gain";
          min = 1;
          max = 8;
          default = 3;
          step = 1;
        }
      ];
    }
    {
      id = "tone";
      family = "tone";
      params = [
        {
          name = "brightness";
          min = -100;
          max = 100;
          default = 0;
          step = 5;
        }
        {
          name = "contrast";
          min = -100;
          max = 100;
          default = 0;
          step = 5;
        }
        {
          name = "saturation";
          min = -100;
          max = 100;
          default = 0;
          step = 5;
        }
        {
          name = "temperature";
          min = -100;
          max = 100;
          default = 0;
          step = 5;
        }
      ];
    }
    {
      id = "pixelate";
      family = "shape";
      params = [
        {
          name = "size";
          min = 2;
          max = 64;
          default = 12;
          step = 2;
        }
      ];
    }
    {
      id = "blur";
      family = "shape";
      params = [
        {
          name = "radius";
          min = 1;
          max = 40;
          default = 8;
          step = 1;
        }
      ];
    }
    {
      id = "sketch";
      family = "shape";
      params = [
        {
          name = "strength";
          min = 1;
          max = 100;
          default = 50;
          step = 5;
        }
      ];
    }
    {
      id = "vignette";
      family = "shape";
      params = [
        {
          name = "strength";
          min = 0;
          max = 100;
          default = 60;
          step = 5;
        }
      ];
    }
    {
      id = "symmetry";
      family = "shape";
      params = [ ];
    }
    {
      id = "kaleidoscope";
      family = "shape";
      params = [
        {
          name = "segments";
          min = 2;
          max = 12;
          default = 6;
          step = 1;
        }
      ];
    }
    {
      id = "chroma-key";
      family = "key";
      params = [
        {
          name = "hue";
          min = 0;
          max = 359;
          default = 120;
          step = 5;
        }
        {
          name = "tolerance";
          min = 1;
          max = 100;
          default = 30;
          step = 1;
        }
      ];
    }
    {
      id = "background-blur";
      family = "background";
      mask = true;
      params = [
        {
          name = "radius";
          min = 1;
          max = 60;
          default = 20;
          step = 1;
        }
        {
          name = "steady";
          min = 0;
          max = 90;
          default = 60;
          step = 5;
        }
      ];
    }
    {
      id = "background-replace";
      family = "background";
      mask = true;
      params = [
        {
          name = "steady";
          min = 0;
          max = 90;
          default = 60;
          step = 5;
        }
      ];
    }
    {
      id = "background-remove";
      family = "background";
      mask = true;
      params = [
        {
          name = "steady";
          min = 0;
          max = 90;
          default = 60;
          step = 5;
        }
      ];
    }
  ];
}
