# cocotb runner for afe_smith_cordic
# Usage:  make            (Icarus)     |     make SIM=verilator
TOPLEVEL_LANG = verilog
VERILOG_SOURCES = $(PWD)/../rtl/afe_smith_cordic.sv
TOPLEVEL = afe_smith_cordic
MODULE   = test_afe_smith_cordic
SIM     ?= icarus

# Icarus needs the SystemVerilog switch
ifeq ($(SIM),icarus)
    COMPILE_ARGS += -g2012
endif

include $(shell cocotb-config --makefiles)/Makefile.sim
