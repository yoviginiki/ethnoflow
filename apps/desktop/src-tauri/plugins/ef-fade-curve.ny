;nyquist plug-in
;version 4
;type process
;preview enabled
;name "EthnoFlow: Cosine fade"
;author "EthnoFlow"
;info "A raised-cosine (S-curve) fade over the whole selection, smoother than the editor's linear fade."
;control direction "Direction" choice "Fade in,Fade out" 0

(setf dur (get-duration 1))

;; Half a cosine period over the selection: 0 → 1 for a fade in, 1 → 0 for a fade out.
(defun curve ()
  (mult 0.5
    (sum 1
      (osc (hz-to-step (/ 0.5 dur)) dur *sine-table* (if (= direction 0) -90 90)))))

(mult *track* (curve))
