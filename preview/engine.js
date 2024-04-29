function displayFrame() {
  const ms = millisecondsSinceStart(window.videoStartedAt)
  console.info("displayFrame", ms)

  if (window.previouslyRenderedFrame) {
    let f = window.frames.get(window.previouslyRenderedFrame)
    f.style.display = "none"
    f.classList.toggle("shown")
  }

  let frame = closestFrame(ms)
  f = window.frames.get(frame)
  f.style.display = "block"
  f.classList.toggle("shown")

  window.previouslyRenderedFrame = frame
}

function closestFrame(ms) {
  const closest = [...window.frames.keys()].reduce((a, b) =>
    Math.abs(b - ms) < Math.abs(a - ms) ? b : a
  )
  console.info("closestFrame", ms, "is", closest)
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

window.addEventListener("keypress", (e) => {
  if (e.key === " ") {
    if (window.intervalID) {
      stopVideo()
    } else {
      startVideo()
    }
  }
})

window.startVideo = () => {
  window.frames = new Map(
    [...document.querySelectorAll("[id^=frame-]")].map((el) => [
      parseInt(el.id.replace("frame-", "")),
      el,
    ])
  )
  window.refreshRate = 50
  window.videoStartedAt = new Date().getTime()
  window.previouslyRenderedFrame = null

  displayFrame()
  window.intervalID = setInterval(displayFrame, window.refreshRate)
}

window.stopVideo = () => {
  console.info("stopVideo", window.currentFrame)
  clearInterval(window.intervalID)
}
