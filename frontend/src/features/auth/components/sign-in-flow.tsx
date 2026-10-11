import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useRouter } from '@tanstack/react-router'
import { AnimatePresence, motion } from 'motion/react'
import { useState } from 'react'
import { spring } from '@/lib/motion'
import { Alert } from '@/components/ui/alert'
import { sessionQuery, signIn, type Credentials } from '@/features/auth/api'
import { signInErrorMessage, USERNAME_PATTERN, WRONG_CREDENTIALS, type SignInStep } from '@/features/auth/messages'
import { AuthCard } from './auth-card'
import { CodeStep } from './code-step'
import { PasswordStep } from './password-step'

type Step = { kind: 'password' } | { kind: 'code'; username: string; password: string }

const copy: Record<Step['kind'], { step: string; title: string; description: string }> = {
  password: {
    step: 'Sign in',
    title: 'Welcome back',
    description: 'Use the username and password a server admin gave you.',
  },
  code: {
    step: 'Step 2 of 2',
    title: 'Two-factor authentication',
    description: 'Your account has two-factor authentication turned on.',
  },
}

// forward slides in from the right and back returns the way it came, so the steps keep their places
const slide = {
  enter: (direction: number) => ({ opacity: 0, x: 24 * direction }),
  center: { opacity: 1, x: 0 },
  exit: (direction: number) => ({ opacity: 0, x: -24 * direction }),
}

export function SignInFlow({ redirectTo }: { redirectTo: string }) {
  const queryClient = useQueryClient()
  const router = useRouter()
  const [step, setStep] = useState<Step>({ kind: 'password' })
  const [username, setUsername] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [codeAttempt, setCodeAttempt] = useState(0)
  const mutation = useMutation({ mutationFn: signIn, meta: { signIn: true } })

  async function attempt(credentials: Credentials, from: SignInStep) {
    setError(null)
    try {
      const result = await mutation.mutateAsync(credentials)
      if (result === 'two_factor_required') {
        setStep({ kind: 'code', username: credentials.username, password: credentials.password })
        return
      }
      await queryClient.query({ ...sessionQuery, staleTime: 0 })
      router.history.replace(redirectTo)
    } catch (cause) {
      setError(signInErrorMessage(cause, from))
      if (from === 'code') setCodeAttempt((count) => count + 1)
    }
  }

  async function submitPassword(normalized: string, password: string) {
    setUsername(normalized)
    // birdd only has users named like this, so anything else is simply wrong
    if (!USERNAME_PATTERN.test(normalized)) {
      setError(WRONG_CREDENTIALS)
      return
    }
    await attempt({ username: normalized, password }, 'password')
  }

  async function submitCode(code: string) {
    if (step.kind !== 'code') return
    await attempt({ username: step.username, password: step.password, code }, 'code')
  }

  function back() {
    setError(null)
    setStep({ kind: 'password' })
  }

  const direction = step.kind === 'code' ? 1 : -1
  const { step: stepLabel, title, description } = copy[step.kind]
  return (
    <AuthCard step={stepLabel} title={title} description={description}>
      <div className="flex flex-col gap-4">
        {error && <Alert key={error}>{error}</Alert>}
        <AnimatePresence mode="popLayout" initial={false} custom={direction}>
          <motion.div
            key={step.kind}
            custom={direction}
            variants={slide}
            initial="enter"
            animate="center"
            exit="exit"
            transition={spring}
          >
            {step.kind === 'password' ? (
              <PasswordStep defaultUsername={username} onSubmit={submitPassword} />
            ) : (
              <CodeStep attempt={codeAttempt} onSubmit={submitCode} onBack={back} />
            )}
          </motion.div>
        </AnimatePresence>
      </div>
      <p className="mt-6 border-t border-line pt-4 text-xs text-fg-faint">No account yet? Ask a server admin to create one.</p>
    </AuthCard>
  )
}
