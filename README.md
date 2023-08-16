# tools7800

This is a set of tools designed as companions to cc7800.

## sprites7800

This is the main tool for generating C code for Atari 7800 graphics (sprites and tiles).
It processes YAML files in input, which references some sprite sheet images :

```
sprite_sheets:
  - image: Bubble Bobble.png
    sprites:
      - name: bb_char1
        top: 0
        left: 0
        width: 16
        holeydma: true
```

which generates (`sprites7800 simple.yaml`)
```
holeydma reversed scattered(16,2) char bb_char1[32] = {
	0x01, 0x00, 0x01, 0x40, 0x0a, 0x94, 0x2a, 0x90, 0x3b, 0xa0, 0xc8, 0xe5, 0xc8, 0xe4, 0xc8, 0xd0,
	0xc8, 0xe5, 0xbb, 0x84, 0x0c, 0x20, 0x2a, 0x90, 0x0e, 0x50, 0x3f, 0x94, 0x3d, 0x68, 0x5d, 0x6a
};
```

Note that the keywords `holeydma`, `reversed` and `scattered` are keywords specific to cc7800
that enable the compiler to properly lay the memory out, interlacing graphics and code as the
Atari 7800 quircky architecture requires. 

Default sprite height is 16 pixels.
cc7800 only supports 8 and 16 pixels high scattered data at the moment.

Default graphics mode is 160A (i.e. double width pixels and 3 colors per sprite + background).
Other graphic mode must be specified with the `mode` attribute.

Main Sprites7800 features :
- All Maria gfx mods are supported (160A, 160B, 320A, 320B, 320C and 320D modes)
- Supports any image format (BMP, JPEG, GIF, PNG, ICO..) 
- Palette definition can be provided to correctly map colors to C code

Note that in 160A and 160B modes, all pixels must be 2 pixels wide (fat pixels) or the image will be rejected.

Sprites7800 was written in Rust language and thus can be easily compiled and installed using Cargo (`cargo install --path .`).

## tiles7800

tiles7800 generates C code from Tiled (Tiles editor) TMX files. It can be used as a simple TMX
to C converter by supplying the TMX file in argument, or it can be used to generate sparse
tiling data C code using the `--sparse` option.

### Sparse tiling

In case of sparse tiling C code generation, you must provide a Sprites7800 YAML containing the tiles definitions
(their mode, the palette number, etc), so that tiles7800 can optimize the generated code and
get benefit of the sparsity of data.

The generated C code must be used with the `sparse_tiling.h` header provided with cc7800.
See the sparse tiling examples in the `examples` directory of cc7800 to see how this works.





