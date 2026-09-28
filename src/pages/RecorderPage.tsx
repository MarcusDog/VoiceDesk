import { Mic2, Pause, Play, ShieldCheck, Square, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Waveform } from '../components/Waveform'
import { appendRecordingChunk, beginRecording, cancelRecording, createProject, enqueueTask, finishRecording, registerMedia, startTranscription } from '../core/client'
import type { CoreConnection, CoreError, RecordingSession } from '../core/types'
import type { PageId } from '../types'

type RecorderPageProps = {
  onNavigate: (page: PageId) => void
  onOpenProject: (projectId: string) => void
  core: CoreConnection
}

const timeLabel = (seconds: number) => [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60].map(value => String(value).padStart(2, '0')).join(':')

export function RecorderPage({ onNavigate, onOpenProject, core }: RecorderPageProps) {
  const [state, setState] = useState<'idle' | 'recording' | 'paused' | 'saving'>('idle')
  const [seconds, setSeconds] = useState(0)
  const [level, setLevel] = useState(0)
  const [autoTranscribe, setAutoTranscribe] = useState(true)
  const [error, setError] = useState('')
  const recorderRef = useRef<MediaRecorder | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const sessionRef = useRef<RecordingSession | null>(null)
  const uploadChainRef = useRef<Promise<void>>(Promise.resolve())
  const audioContextRef = useRef<AudioContext | null>(null)
  const frameRef = useRef(0)

  useEffect(() => {
    if (state !== 'recording') return
    const timer = window.setInterval(() => setSeconds(value => value + 1), 1000)
    return () => window.clearInterval(timer)
  }, [state])

  useEffect(() => () => {
    window.cancelAnimationFrame(frameRef.current)
    streamRef.current?.getTracks().forEach(track => track.stop())
    void audioContextRef.current?.close()
  }, [])

  const stopCapture = () => {
    window.cancelAnimationFrame(frameRef.current)
    setLevel(0)
    streamRef.current?.getTracks().forEach(track => track.stop())
    streamRef.current = null
    void audioContextRef.current?.close()
    audioContextRef.current = null
  }

  const beginMeter = (stream: MediaStream) => {
    const context = new AudioContext()
    const analyser = context.createAnalyser()
    analyser.fftSize = 256
    context.createMediaStreamSource(stream).connect(analyser)
    audioContextRef.current = context
    const values = new Uint8Array(analyser.fftSize)
    const render = () => {
      analyser.getByteTimeDomainData(values)
      const rms = Math.sqrt(values.reduce((sum, value) => sum + ((value - 128) / 128) ** 2, 0) / values.length)
      setLevel(Math.min(16, Math.max(1, Math.round(rms * 42))))
      frameRef.current = window.requestAnimationFrame(render)
    }
    render()
  }

  const start = async () => {
    if (core.mode !== 'desktop') {
      setError('真实录音需要使用 npm run desktop:dev 启动桌面端。')
      return
    }
    setError('')
    try {
      const session = await beginRecording()
      sessionRef.current = session
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      })
      const preferred = 'audio/webm;codecs=opus'
      const recorder = new MediaRecorder(stream, MediaRecorder.isTypeSupported(preferred) ? { mimeType: preferred } : undefined)
      streamRef.current = stream
      recorderRef.current = recorder
      uploadChainRef.current = Promise.resolve()
      recorder.ondataavailable = event => {
        if (!event.data.size || !sessionRef.current) return
        const recordingId = sessionRef.current.id
        uploadChainRef.current = uploadChainRef.current.then(async () => {
          const bytes = new Uint8Array(await event.data.arrayBuffer())
          await appendRecordingChunk(recordingId, bytes)
        })
      }
      recorder.onerror = () => setError('麦克风录制发生错误，请检查设备。')
      recorder.start(1000)
      beginMeter(stream)
      setSeconds(0)
      setState('recording')
    } catch (value) {
      const message = (value as CoreError)?.message || (value instanceof Error ? value.message : '无法开始录音。')
      setError(message.includes('Permission') || message.includes('NotAllowed') ? '麦克风权限被拒绝，请在 Windows 隐私设置中允许麦克风访问。' : message)
      if (sessionRef.current) await cancelRecording(sessionRef.current.id).catch(() => undefined)
      sessionRef.current = null
      stopCapture()
    }
  }

  const waitForStop = (recorder: MediaRecorder) => new Promise<void>(resolve => {
    if (recorder.state === 'inactive') return resolve()
    recorder.addEventListener('stop', () => resolve(), { once: true })
    recorder.stop()
  })

  const finish = async () => {
    const recorder = recorderRef.current
    const session = sessionRef.current
    if (!recorder || !session) return
    setState('saving')
    setError('')
    try {
      await waitForStop(recorder)
      await uploadChainRef.current
      const probe = await finishRecording(session.id)
      stopCapture()
      sessionRef.current = null
      recorderRef.current = null
      if (!autoTranscribe) {
        setState('idle')
        setSeconds(0)
        onNavigate('library')
        return
      }
      const project = await createProject(session.displayName.replace(/\.webm$/i, ''))
      await registerMedia(project.id, probe)
      const task = await enqueueTask('transcription', project.id, {
        mediaPaths: [probe.path], model: 'small', language: 'zh', device: 'auto', computeType: 'auto', beamSize: 5,
      })
      await startTranscription(task.id)
      onOpenProject(project.id)
    } catch (value) {
      setState('idle')
      setError((value as CoreError)?.message || '保存录音或启动转写失败。')
      stopCapture()
    }
  }

  const togglePause = () => {
    const recorder = recorderRef.current
    if (!recorder) return
    if (state === 'paused') {
      recorder.resume()
      void audioContextRef.current?.resume()
      setState('recording')
    } else if (state === 'recording') {
      recorder.pause()
      void audioContextRef.current?.suspend()
      setState('paused')
    }
  }

  const cancel = async () => {
    const recorder = recorderRef.current
    const session = sessionRef.current
    if (recorder) await waitForStop(recorder)
    await uploadChainRef.current.catch(() => undefined)
    if (session) await cancelRecording(session.id).catch(() => undefined)
    sessionRef.current = null
    recorderRef.current = null
    stopCapture()
    setState('idle')
    setSeconds(0)
    setError('')
  }

  return (
    <div className="page recorder-page">
      <div className="page-heading">
        <span className="section-index">03</span>
        <h1>录音</h1>
      </div>
      <section className="recorder-card">
        <div className="recorder-status"><span className={state === 'recording' ? 'is-live' : ''}><i /> {state === 'recording' ? '正在录音' : state === 'paused' ? '录音已暂停' : state === 'saving' ? '正在保存' : '准备就绪'}</span><em><ShieldCheck size={15} /> 本地录制 · 隐私安全</em></div>
        <div className="timer">{timeLabel(seconds)}</div>
        <p className="estimated-size">预计大小 · {(seconds * 0.017).toFixed(1)} MB</p>
        <Waveform active={state === 'idle' ? 0 : state === 'paused' ? 4 : Math.max(8, level * 6)} />
        <div className="input-panel">
          <label><Mic2 size={16} /> 麦克风设备<select defaultValue="系统默认麦克风" disabled={state !== 'idle'}><option>系统默认麦克风</option></select></label>
          <div className="level-meter"><span>实时输入音量</span><div>{Array.from({ length: 16 }).map((_, index) => <i key={index} className={index < level ? 'is-on' : ''} />)}</div></div>
        </div>
        <div className="recorder-controls">
          <button className="control-button" disabled={state === 'idle' || state === 'saving'} onClick={togglePause}>{state === 'paused' ? <Play /> : <Pause />}<strong>{state === 'paused' ? '继续' : '暂停'}</strong><small>保留当前录音</small></button>
          <button className={`record-button ${state === 'recording' ? 'is-recording' : ''}`} disabled={state === 'saving'} onClick={() => state === 'idle' ? void start() : void finish()}>{state === 'idle' ? <Mic2 /> : <Square fill="currentColor" />}<strong>{state === 'idle' ? '开始录音' : state === 'saving' ? '正在保存' : '结束录音'}</strong><small>{state === 'idle' ? '允许麦克风权限' : '保存并转写'}</small></button>
          <button className="control-button" disabled={state === 'idle' || state === 'saving'} onClick={() => void cancel()}><X /><strong>取消</strong><small>删除本次录音</small></button>
        </div>
        {error && <div className="form-error" role="alert">{error}</div>}
      </section>
      <label className="setting-row"><span><Mic2 size={19} /><span><strong>录音完成后自动转写</strong><small>保存音频后，自动进入本地 ASR 流程</small></span></span><input type="checkbox" checked={autoTranscribe} onChange={event => setAutoTranscribe(event.target.checked)} /><i /></label>
    </div>
  )
}
