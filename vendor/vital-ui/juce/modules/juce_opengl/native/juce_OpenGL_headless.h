/*
  Rind Studio / Honeydew: an OpenGL context for JUCE that draws into memory (EGL + OpenGL ES 3, no window).

  The context is created on the render thread; it draws into a framebuffer object the size of the component, and
  after every swap hands the pixels (RGBA, top row first) to the embedding app's frame callback. Works with Mesa
  (surfaceless platform, for tests) and Android's EGL alike. GPL-3.0-or-later like the rest of the tree.
*/

namespace juce
{

/** Called on the render thread after every frame with the finished picture (valid for the call only). */
std::function<void (int width, int height, const uint8* rgbaTopDown)> juce_headlessFrameCallback;

/** The most frames per second the context renders (the swap blocks for the rest of the frame). */
std::atomic<int> juce_headlessMaxFps { 30 };

class OpenGLContext::NativeContext
{
public:
    NativeContext (Component& comp, const OpenGLPixelFormat&, void* shareContext, bool, OpenGLVersion)
        : component (comp), contextToShareWith (shareContext)
    {
        updateWindowPosition (component.getTopLevelComponent()->getLocalArea (&component, component.getLocalBounds()));
    }

    ~NativeContext() = default;

    bool initialiseOnRenderThread (OpenGLContext& c)
    {
        if (! initialiseDisplay())
            return false;

        const EGLint configAttribs[] = { EGL_SURFACE_TYPE, EGL_PBUFFER_BIT,
                                         EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT,
                                         EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8,
                                         EGL_NONE };
        EGLint numConfigs = 0;

        if (! eglChooseConfig (sharedDisplay(), configAttribs, &config, 1, &numConfigs) || numConfigs < 1)
            return false;

        const EGLint contextAttribs[] = { EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE };
        context = eglCreateContext (sharedDisplay(), config, contextToShareWith != nullptr ? (EGLContext) contextToShareWith
                                                                                           : EGL_NO_CONTEXT,
                                    contextAttribs);
        if (context == EGL_NO_CONTEXT)
            return false;

        const EGLint pbufferAttribs[] = { EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE };
        surface = eglCreatePbufferSurface (sharedDisplay(), config, pbufferAttribs);

        juceContext = &c;
        c.makeActive();
        return true;
    }

    void shutdownOnRenderThread()
    {
        juceContext = nullptr;
        deactivateCurrentContext();
        destroyTarget();

        if (surface != EGL_NO_SURFACE)
            eglDestroySurface (sharedDisplay(), surface);

        if (context != EGL_NO_CONTEXT)
            eglDestroyContext (sharedDisplay(), context);

        surface = EGL_NO_SURFACE;
        context = EGL_NO_CONTEXT;
    }

    bool makeActive() const noexcept
    {
        if (context == EGL_NO_CONTEXT || ! eglMakeCurrent (sharedDisplay(), surface, surface, context))
            return false;

        const_cast<NativeContext*> (this)->ensureTarget();
        glBindFramebuffer (GL_FRAMEBUFFER, frameBuffer);
        return true;
    }

    bool isActive() const noexcept              { return context != EGL_NO_CONTEXT && eglGetCurrentContext() == context; }

    static void deactivateCurrentContext()
    {
        if (sharedDisplay() != EGL_NO_DISPLAY)
            eglMakeCurrent (sharedDisplay(), EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
    }

    void swapBuffers()
    {
        if (frameBuffer != 0 && juce_headlessFrameCallback)
        {
            glBindFramebuffer (GL_FRAMEBUFFER, frameBuffer);
            glPixelStorei (GL_PACK_ALIGNMENT, 1);
            pixels.resize ((size_t) width * (size_t) height * 4);
            glReadPixels (0, 0, width, height, GL_RGBA, GL_UNSIGNED_BYTE, pixels.data());

            // OpenGL's rows run bottom to top.
            flipped.resize (pixels.size());
            const size_t row = (size_t) width * 4;

            for (int y = 0; y < height; ++y)
                std::memcpy (flipped.data() + (size_t) y * row, pixels.data() + (size_t) (height - 1 - y) * row, row);

            juce_headlessFrameCallback (width, height, flipped.data());
        }
        else
        {
            glFinish();
        }

        const auto period = (uint32) (1000 / jmax (1, juce_headlessMaxFps.load()));
        const auto now = Time::getMillisecondCounter();

        if (now - lastSwap < period)
            Thread::sleep ((int) (period - (now - lastSwap)));

        lastSwap = Time::getMillisecondCounter();
    }

    void updateWindowPosition (Rectangle<int> newBounds)
    {
        const auto scale = (float) component.getDesktopScaleFactor();
        wantedWidth  = jmax (1, (int) std::lround (newBounds.getWidth()  * scale));
        wantedHeight = jmax (1, (int) std::lround (newBounds.getHeight() * scale));
    }

    bool setSwapInterval (int numFramesPerSwap)     { swapFrames = numFramesPerSwap; return true; }
    int getSwapInterval() const                     { return swapFrames; }
    bool createdOk() const noexcept                 { return true; }
    void* getRawContext() const noexcept            { return (void*) context; }
    GLuint getFrameBufferID() const noexcept        { return frameBuffer; }

    void triggerRepaint()
    {
        if (juceContext != nullptr)
            juceContext->triggerRepaint();
    }

    struct Locker { Locker (NativeContext&) {} };

private:
    static EGLDisplay& sharedDisplay() noexcept
    {
        static EGLDisplay display = EGL_NO_DISPLAY;
        return display;
    }

    static bool initialiseDisplay()
    {
        static std::mutex lock;
        std::lock_guard<std::mutex> guard (lock);

        if (sharedDisplay() != EGL_NO_DISPLAY)
            return true;

        EGLDisplay display = EGL_NO_DISPLAY;

       #if ! defined (__ANDROID__)
        // No window system: Mesa's surfaceless platform (what tests and CI machines have).
        using GetPlatformDisplay = EGLDisplay (*) (EGLenum, void*, const EGLint*);

        if (auto getPlatformDisplay = (GetPlatformDisplay) eglGetProcAddress ("eglGetPlatformDisplayEXT"))
            display = getPlatformDisplay (0x31DD /* EGL_PLATFORM_SURFACELESS_MESA */, EGL_DEFAULT_DISPLAY, nullptr);
       #endif

        if (display == EGL_NO_DISPLAY)
            display = eglGetDisplay (EGL_DEFAULT_DISPLAY);

        EGLint major = 0, minor = 0;

        if (display == EGL_NO_DISPLAY || ! eglInitialize (display, &major, &minor))
            return false;

        eglBindAPI (EGL_OPENGL_ES_API);
        sharedDisplay() = display;
        return true;
    }

    /** (render thread) The framebuffer the frame is drawn into, remade when the component's size changed. */
    void ensureTarget()
    {
        if (frameBuffer != 0 && wantedWidth == width && wantedHeight == height)
            return;

        destroyTarget();
        width = wantedWidth;
        height = wantedHeight;

        glGenRenderbuffers (1, &colourBuffer);
        glBindRenderbuffer (GL_RENDERBUFFER, colourBuffer);
        glRenderbufferStorage (GL_RENDERBUFFER, GL_RGBA8, width, height);

        glGenRenderbuffers (1, &depthBuffer);
        glBindRenderbuffer (GL_RENDERBUFFER, depthBuffer);
        glRenderbufferStorage (GL_RENDERBUFFER, GL_DEPTH24_STENCIL8, width, height);

        glGenFramebuffers (1, &frameBuffer);
        glBindFramebuffer (GL_FRAMEBUFFER, frameBuffer);
        glFramebufferRenderbuffer (GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_RENDERBUFFER, colourBuffer);
        glFramebufferRenderbuffer (GL_FRAMEBUFFER, GL_DEPTH_STENCIL_ATTACHMENT, GL_RENDERBUFFER, depthBuffer);
        jassert (glCheckFramebufferStatus (GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE);
    }

    void destroyTarget()
    {
        if (frameBuffer != 0)   { glDeleteFramebuffers (1, &frameBuffer);    frameBuffer = 0; }
        if (colourBuffer != 0)  { glDeleteRenderbuffers (1, &colourBuffer);  colourBuffer = 0; }
        if (depthBuffer != 0)   { glDeleteRenderbuffers (1, &depthBuffer);   depthBuffer = 0; }
    }

    Component& component;
    void* contextToShareWith;
    EGLConfig config = nullptr;
    EGLContext context = EGL_NO_CONTEXT;
    EGLSurface surface = EGL_NO_SURFACE;
    OpenGLContext* juceContext = nullptr;

    std::atomic<int> wantedWidth { 1 }, wantedHeight { 1 };
    int width = 0, height = 0, swapFrames = 1;
    GLuint frameBuffer = 0, colourBuffer = 0, depthBuffer = 0;
    std::vector<uint8> pixels, flipped;
    uint32 lastSwap = 0;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (NativeContext)
};

//==============================================================================
bool OpenGLHelpers::isContextActive()
{
    return eglGetCurrentContext() != EGL_NO_CONTEXT;
}

} // namespace juce
