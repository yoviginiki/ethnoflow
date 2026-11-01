;nyquist plug-in
;version 4
;type process
;preview enabled
;name "EthnoFlow: Tremolo"
;author "EthnoFlow"
;info "Amplitude wobble: a low-frequency oscillator scales the level between (1 - depth) and 1."
;control depth "Depth (%)" int "%" 40 1 100
;control rate "Rate (Hz)" float "Hz" 5 0.1 20
;control shape "Shape" choice "Sine,Triangle" 0

(setf level (/ depth 100.0))
(setf dur (get-duration 1))

;; An LFO goes from -1 to 1; shift it to 0..1, then into (1 - level)..1.
(defun wobble (sig)
  (let* ((table (if (= shape 0) *sine-table* *tri-table*))
         (osc01 (mult 0.5 (sum 1 (lfo rate dur table))))
         (env (sum (- 1 level) (mult level osc01))))
    (mult sig env)))

(multichan-expand #'wobble *track*)
