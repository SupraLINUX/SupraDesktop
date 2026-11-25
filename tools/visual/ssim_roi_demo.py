"""Synthetic ROI sanity check; not a macOS reference or graphics benchmark."""
import numpy as np
from skimage.metrics import structural_similarity

reference = np.full((512, 512), 0.5, dtype=np.float64)
candidate = reference.copy()
candidate[240:272, 240:272] = 0.9
roi = np.s_[232:280, 232:280]
print("Global:", structural_similarity(reference, candidate, data_range=1.0))
print("ROI:", structural_similarity(reference[roi], candidate[roi], data_range=1.0))
print("Changed pixels:", np.count_nonzero(reference != candidate))
