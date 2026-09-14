#ifndef COHERENCE_H
#define COHERENCE_H

#include <stdint.h>
#include <stdbool.h>

typedef struct {
    double phi_c;
    double phi_delta;
    double ratio;
    double entropy;
} CoherenceState;

#endif /* COHERENCE_H */
