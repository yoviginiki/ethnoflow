;nyquist plug-in
;version 4
;type process
;name "EthnoFlow: Gain (dB)"
;author "EthnoFlow"
;info "Changes the level of the selection by a number of decibels. The simplest possible plug-in: a good first file to copy and change."
;control gain "Gain (dB)" float "dB" -6 -36 12

;; `*track*` is the selection (a sound, or an array of two sounds for stereo);
;; every sound primitive works on both. The last value is the result.
(scale-db gain *track*)
