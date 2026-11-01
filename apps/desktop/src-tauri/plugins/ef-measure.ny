;nyquist plug-in
;version 4
;type analyze
;name "EthnoFlow: Measure level"
;author "EthnoFlow"
;info "Reports the peak and RMS level of the selection and how much gain would bring the peak to the target."
;control target "Target peak (dBFS)" float "dBFS" -1 -24 0

(let* ((peak (get '*selection* 'peak))
       (rms (get '*selection* 'rms))
       (peak-db (linear-to-db (max peak 1e-9)))
       (rms-db (linear-to-db (max rms 1e-9))))
  (format nil "Peak ~a dBFS, RMS ~a dBFS, ~a channels, ~a s.~%Gain to reach ~a dBFS: ~a dB."
          (/ (round (* 10 peak-db)) 10.0)
          (/ (round (* 10 rms-db)) 10.0)
          (get '*track* 'channels)
          (/ (round (* 100 (get-duration 1))) 100.0)
          target
          (/ (round (* 10 (- target peak-db))) 10.0)))
