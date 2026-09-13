import unittest

from compare_services import fairness_gaps, summary


class ComparisonEvidenceTests(unittest.TestCase):
    def test_two_unknown_configs_do_not_establish_parity(self):
        self.assertTrue(fairness_gaps({'anybranch': {}, 'ardent': {}}))

    def test_failures_remain_in_denominator(self):
        result = summary([{'success': True, 'rw': 10}, {'success': False},
                          {'success': True, 'rw': 30}], 'rw')
        self.assertEqual(result['attempts'], 3)
        self.assertEqual(result['failures'], 1)
        self.assertEqual(result['p50_ms'], 20)
        self.assertEqual(result['p95_ms'], 30)

    def test_all_failures_produce_no_latency(self):
        result = summary([{'success': False}], 'rw')
        self.assertNotIn('p50_ms', result)


if __name__ == '__main__':
    unittest.main()
