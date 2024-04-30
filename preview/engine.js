/**
 * @typedef {object} GlobalData
 * @property {Map<number, HTMLDivElement>} [frames]
 */

/**
 * @typedef {Window & GlobalData} WindowWithData
 */

function displayFrame() {
  const ms = millisecondsSinceStart(window.videoStartedAt)
  console.debug("displayFrame", ms)

  if (window.previouslyRenderedFrame) {
    let f = window.frames.get(window.previouslyRenderedFrame)
    if (f) {
      f.style.display = "none"
      f.classList.toggle("shown")
    }
  }

  let frame = closestFrame(ms)
  f = window.frames.get(frame)
  f.style.display = "block"
  f.classList.toggle("shown")

  console.debug("framestLeftCount", framesLeftCount())
  if (framesLeftCount() < window.FRAMES_BUFFER_SIZE) {
    console.warn(
      window.updatingBuffer ? "already updating buffer" : "update buffer now"
    )

    if (
      !window.updatingBuffer &&
      window.previouslyRenderedFrame - window.lastBufferUpdateWasOn > 2000
    ) {
      console.info(
        `Updating buffer: ${framesLeftCount()} < ${
          window.FRAMES_BUFFER_SIZE
        } remaining and last update was ${
          window.previouslyRenderedFrame - window.lastBufferUpdateWasOn
        } > 2000 ms ago`
      )
      updateBuffer()
    }
  }

  window.previouslyRenderedFrame = frame
}

/**
 *
 * @returns number
 */
function framesLeftCount() {
  return [...window.frames.keys()].filter(
    (key) => key > window.previouslyRenderedFrame
  ).length
}

/**
 *
 * @param {number} ms
 * @returns {number}
 */
function closestFrame(ms) {
  const closest = [...window.frames.keys()].reduce((a, b) =>
    Math.abs(b - ms) < Math.abs(a - ms) ? b : a
  )
  console.debug("closestFrame", ms, "is", closest)
  return closest
}

/**
 *
 * @param {number} start
 * @returns
 */
function millisecondsSinceStart(start) {
  return new Date().getTime() - start
}

/**
 *
 * @returns {number}
 */
function lastLoadedFrame() {
  return Math.max(...[...window.frames.keys()])
}

async function updateBuffer() {
  console.time("fetchFrames")
  console.log("set updatingBuffer to true")
  window.updatingBuffer = true
  console.info(
    "updateBuffer",
    4 * window.FRAMES_BUFFER_SIZE,
    window.previouslyRenderedFrame
  )
  // request half the buffer size for the next frames
  await fetch(
    new URL(
      "/frames?" +
        new URLSearchParams({
          next: 4 * window.FRAMES_BUFFER_SIZE,
          from: window.previouslyRenderedFrame,
        }),
      window.SERVER_ORIGIN
    )
  ).then((response) => {
    if (!response.ok) {
      console.error("Failed to fetch frames", response)
      return
    }
    console.timeEnd("fetchFrames")

    console.time("insertFramesToDOM")
    response.text().then((frames) => {
      document.body.insertAdjacentHTML("beforeend", frames)
    })
    console.timeEnd("insertFramesToDOM")
  })

  console.time("pruneFramesFromDOM")
  // remove frames that are not needed anymore
  ;[...window.frames.keys()].forEach((key) => {
    if (key < window.previouslyRenderedFrame) {
      window.frames.get(key).remove()
    }
  })
  console.timeEnd("pruneFramesFromDOM")

  loadFramesFromDOM()
  console.log("set updatingBuffer to false")
  window.updatingBuffer = false
  window.lastBufferUpdateWasOn = window.previouslyRenderedFrame
}

window.addEventListener("keypress", (e) => {
  if (e.key === " ") {
    if (window.intervalID) {
      stopVideo()
    } else {
      startVideo()
    }
  }
})

function loadFramesFromDOM() {
  console.time("loadFramesFromDOM")
  window.frames = new Map(
    [...document.querySelectorAll("[id^=frame-]")].map((el) => [
      parseInt(el.id.replace("frame-", "")),
      el,
    ])
  )
  console.timeEnd("loadFramesFromDOM")
}

window.startVideo = () => {
  loadFramesFromDOM()
  window.refreshRate = 50
  window.videoStartedAt = new Date().getTime()
  window.previouslyRenderedFrame = null
  window.lastBufferUpdateWasOn = null
  window.updatingBuffer = false

  displayFrame()
  window.intervalID = setInterval(displayFrame, window.refreshRate)
}

window.stopVideo = () => {
  console.info("stopVideo", window.currentFrame)
  clearInterval(window.intervalID)
}
