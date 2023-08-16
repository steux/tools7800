# tiles7800

tiles7800 generates C code from Tiled (Tiles editor) TMX files. It can be used as a simple TMX
to C converter by supplying the TMX file in argument, or it can be used to generate sparse
tiling data C code using the `--sparse` option.

## Sparse tiling

In case of sparse tiling C code generation, you must provide a Sprites7800 YAML containing the tiles definitions
(their mode, the palette number, etc), so that tiles7800 can optimize the generated code and
get benefit of the sparsity of data.

The generated C code must be used with the `sparse_tiling.h` header provided with cc7800.
See the sparse tiling examples in the `examples` directory of cc7800 to see how this works.





