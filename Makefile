.SUFFIXES:

.PHONY: all clean

outputs:=security_bins/japan_to_usa.bin security_bins/japan_to_europe.bin security_bins/europe_to_usa.bin \
	security_bins/europe_to_japan.bin security_bins/usa_to_europe.bin security_bins/usa_to_japan.bin

all: $(outputs)

clean:
	rm -rf $(outputs)

security_bins/%.bin : adapter_src/%.s68 adapter_src/defs.inc
	motor68k -fb -a2 -o$@ $<