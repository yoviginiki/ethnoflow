;nyquist plug-in
;version 4
;type generate
;preview enabled
;name "EthnoFlow: Tone"
;author "EthnoFlow"
;info "A generator to copy from: a plucked string or a soft pad at a pitch, with its own envelope. Generators get no input: they build the sound from nothing and return it."
;control pitch "Pitch (MIDI note, 62 = D4)" int "" 62 36 96
;control dur "Duration (s)" float "s" 2 0.1 20
;control kind "Sound" choice "Pluck,Pad" 0
;control level "Level (dB)" float "dB" -6 -36 0

;; `osc` takes a MIDI step; `pluck` is a Karplus-Strong string; `pwl` is the envelope.
(defun pad ()
  (mult (pwl 0.3 1 (- dur 0.5) 1 dur)
        (sim (osc pitch dur)
             (scale 0.4 (osc (+ pitch 12) dur))
             (scale 0.25 (osc (+ pitch 19) dur)))))

(scale-db level
  (if (= kind 0)
      (pluck pitch dur 0.01)
      (pad)))
